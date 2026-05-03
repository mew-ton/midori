mod error;
mod plugin_resolver;
mod profile;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::error::CliError;
use crate::plugin_resolver::{resolve_drivers, ResolvedDrivers};
use crate::profile::{collect_driver_names, load_from_path as load_profile};
use midori_runtime::events_pipeline::{check_driver_schema, DriverSchemaOutcome};
use midori_runtime::logging;
use midori_runtime::logging::{LogFormat, LogLevel};

#[derive(Parser, Debug)]
#[command(
    name = "midori",
    version,
    about = "Midori signal bridge CLI",
    propagate_version = true
)]
struct Cli {
    /// アプリデータディレクトリ。省略時は OS 標準の場所を使用する
    #[arg(long, value_name = "PATH", global = true)]
    app_data_dir: Option<PathBuf>,

    /// stdout に出力するログのレベル
    #[arg(long, value_enum, default_value_t = LogLevel::Info, global = true)]
    log_level: LogLevel,

    /// stdout に出力するログのフォーマット
    #[arg(long, value_enum, default_value_t = LogFormat::Json, global = true)]
    log_format: LogFormat,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// プロファイル YAML を読み込んでパイプラインを起動する
    Run {
        /// プロファイル YAML へのパス
        #[arg(value_name = "PROFILE")]
        profile: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    logging::init(cli.log_level, cli.log_format);
    match dispatch(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            logging::error("bridge", None, err);
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: &Cli) -> Result<(), CliError> {
    match &cli.command {
        Command::Run { profile } => run(profile, cli.app_data_dir.as_deref()),
    }
}

// プロファイル本体のパイプラインは後続の subtask で実装する。
// 本関数では (a) profile YAML を読み、(b) inputs/outputs から driver 名を
// 抽出して (c) `<app-data-dir>/plugins/*/.midori/plugin.yaml` ベースで
// driver manifest を解決し、(d) 各 driver の events.yaml を起動時整合性
// チェックする、までを担う。実 driver process spawn / SPSC 連携は別 subtask
// の責務。
fn run(profile_path: &Path, app_data_dir_override: Option<&Path>) -> Result<(), CliError> {
    let profile = load_profile(profile_path).map_err(|source| CliError::LoadProfile { source })?;
    let driver_names = collect_driver_names(&profile);
    let app_data_dir = resolve_app_data_dir(app_data_dir_override)?;
    let resolved: ResolvedDrivers =
        resolve_drivers(&app_data_dir).map_err(|source| CliError::ResolvePlugins { source })?;

    for name in &driver_names {
        let Some(driver) = resolved.get(name) else {
            return Err(CliError::DriverNotInstalled {
                driver_name: name.clone(),
            });
        };
        let events_yaml_path = driver.events_yaml_path();
        match check_driver_schema(name, &events_yaml_path)
            .map_err(|source| CliError::StartupCheck { source })?
        {
            DriverSchemaOutcome::Loaded(_) => {
                logging::info(
                    "bridge",
                    Some(name),
                    format_args!(
                        "events.yaml ({}) のチェックが完了しました",
                        events_yaml_path.display()
                    ),
                );
            }
            DriverSchemaOutcome::Missing => {
                // events.yaml が無い driver は spec 上「明示的な schema 未宣言モード」
                // として warning に留めて起動を継続する。
                logging::warn(
                    "bridge",
                    Some(name),
                    format_args!(
                        "events.yaml ({}) が見つかりませんでした。schema 未宣言モードで起動します",
                        events_yaml_path.display()
                    ),
                );
            }
        }
    }

    let _ = profile; // adapter / transform の本格ロードは別 subtask
    Ok(())
}

/// CLI override が無ければ OS 標準のアプリデータディレクトリを `dirs` 経由
/// で解決する。`design/04-runtime-cli.md` の表に従い:
/// - macOS: `~/Library/Application Support/Midori`
/// - Windows: `%APPDATA%\Midori`
/// - Linux: `$XDG_DATA_HOME/midori`（未設定時 `~/.local/share/midori`）
fn resolve_app_data_dir(cli_override: Option<&Path>) -> Result<PathBuf, CliError> {
    if let Some(path) = cli_override {
        return Ok(path.to_path_buf());
    }
    let base = dirs::data_dir().ok_or(CliError::AppDataDirUnavailable)?;
    Ok(
        base.join(if cfg!(any(target_os = "macos", target_os = "windows")) {
            "Midori"
        } else {
            "midori"
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, LogFormat, LogLevel};
    use clap::{CommandFactory, Parser};
    use std::path::{Path, PathBuf};

    #[test]
    fn it_should_render_help_listing_run_subcommand() {
        let mut cmd = Cli::command();
        let help = cmd.render_long_help().to_string();
        assert!(help.contains("run"), "help should list the run subcommand");
        assert!(
            help.contains("--app-data-dir"),
            "help should list --app-data-dir"
        );
        assert!(help.contains("--log-level"), "help should list --log-level");
        assert!(
            help.contains("--log-format"),
            "help should list --log-format"
        );
    }

    #[test]
    fn it_should_parse_run_with_profile_path() {
        let cli = Cli::try_parse_from(["midori", "run", "/tmp/profile.yaml"])
            .expect("run subcommand with positional profile must parse");

        match cli.command {
            Command::Run { profile } => {
                assert_eq!(profile, PathBuf::from("/tmp/profile.yaml"));
            }
        }
    }

    #[test]
    fn it_should_default_log_options_to_info_and_json() {
        let cli = Cli::try_parse_from(["midori", "run", "p.yaml"])
            .expect("default log options must apply");
        assert_eq!(cli.log_level, LogLevel::Info);
        assert_eq!(cli.log_format, LogFormat::Json);
    }

    #[test]
    fn it_should_accept_global_options_before_subcommand() {
        let cli = Cli::try_parse_from([
            "midori",
            "--log-level",
            "debug",
            "--log-format",
            "text",
            "--app-data-dir",
            "/var/midori",
            "run",
            "p.yaml",
        ])
        .expect("global options before subcommand must parse");
        assert_eq!(cli.log_level, LogLevel::Debug);
        assert_eq!(cli.log_format, LogFormat::Text);
        assert_eq!(cli.app_data_dir, Some(PathBuf::from("/var/midori")));
    }

    #[test]
    fn it_should_reject_run_without_profile_argument() {
        let result = Cli::try_parse_from(["midori", "run"]);
        assert!(result.is_err(), "run requires a positional profile arg");
    }

    #[test]
    fn it_should_fail_when_profile_file_is_missing() {
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            "/nonexistent/midori-runtime-test/profile.yaml",
        ])
        .expect("parse");
        let result = super::dispatch(&cli);
        assert!(result.is_err(), "missing profile should fail");
    }

    // --------------------------------------------------------------
    // profile YAML → events.yaml startup chain
    // --------------------------------------------------------------

    use crate::error::CliError;

    /// 与えた `(kind, name)` ペアのリストから profile YAML を組み立てて
    /// tempfile に書き出す。`kind == "input"` のものを `inputs[]`、
    /// `kind == "output"` のものを `outputs[]` に振り分ける。spec 上 inputs
    /// / outputs はそれぞれ最低 1 件必要なので、片方が空のときは dummy
    /// エントリを補って `ProfileLoadError::Invalid` を回避する。
    fn write_tmp_profile(tag: &str, drivers: &[(&str, &str)]) -> tempfile::NamedTempFile {
        // drivers: (kind, name) where kind は "input" / "output"
        let mut yaml = String::from("inputs:\n");
        let mut input_count = 0;
        for (kind, name) in drivers {
            if *kind == "input" {
                use std::fmt::Write as _;
                writeln!(
                    yaml,
                    "  - adapter: adapters/{name}.yaml\n    connection:\n      driver: {name}"
                )
                .expect("string write");
                input_count += 1;
            }
        }
        if input_count == 0 {
            // input が無いと invalid。テスト都合で空の dummy input を入れる。
            yaml.push_str(
                "  - adapter: adapters/dummy-in.yaml\n    connection:\n      driver: dummy-in\n",
            );
        }
        yaml.push_str("transform: mappers/example.yaml\n");
        yaml.push_str("outputs:\n");
        let mut output_count = 0;
        for (kind, name) in drivers {
            if *kind == "output" {
                use std::fmt::Write as _;
                writeln!(
                    yaml,
                    "  - adapter: adapters/{name}.yaml\n    connection:\n      driver: {name}"
                )
                .expect("string write");
                output_count += 1;
            }
        }
        if output_count == 0 {
            yaml.push_str(
                "  - adapter: adapters/dummy-out.yaml\n    connection:\n      driver: dummy-out\n",
            );
        }
        let mut file = tempfile::Builder::new()
            .prefix(&format!("midori-profile-test-{tag}-"))
            .suffix(".yaml")
            .tempfile()
            .expect("tempfile");
        std::io::Write::write_all(&mut file, yaml.as_bytes()).expect("write profile");
        file
    }

    /// `<app-data-dir>/plugins/<plugin>/.midori/plugin.yaml` +
    /// `<app-data-dir>/plugins/<plugin>/drivers/<driver>/driver.yaml` +
    /// `events.yaml` の三層構造を tempdir に用意する。`bodies` で
    /// `name -> events.yaml 内容` を渡す（プラグイン名は `<name>-plugin`、
    /// driver サブディレクトリ名は `<name>` と固定する）。
    fn setup_app_data_dir(bodies: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::Builder::new()
            .prefix("midori-profile-test-app-")
            .tempdir()
            .expect("tempdir");
        for (name, events_body) in bodies {
            let plugin_root = dir.path().join("plugins").join(format!("{name}-plugin"));
            let dot_midori = plugin_root.join(".midori");
            std::fs::create_dir_all(&dot_midori).expect("mkdir .midori");
            let plugin_yaml = format!(
                "name: {name}-plugin\ndrivers:\n  - driver: ../drivers/{name}/driver.yaml\n"
            );
            std::fs::write(dot_midori.join("plugin.yaml"), plugin_yaml).expect("write plugin.yaml");

            let driver_dir = plugin_root.join("drivers").join(name);
            std::fs::create_dir_all(&driver_dir).expect("mkdir driver");
            let driver_yaml = format!("name: {name}\nmodality: {name}\n");
            std::fs::write(driver_dir.join("driver.yaml"), driver_yaml).expect("write driver.yaml");
            std::fs::write(driver_dir.join("events.yaml"), events_body).expect("write events.yaml");
        }
        dir
    }

    /// 正常通過する最小 events.yaml フィクスチャ（noteOn の range が
    /// `[1, 16]` で validator / feature check を全て通過する）。
    const EVENTS_YAML_VALID_NOTEON: &str = "schema_version: 1\n\
         events:\n  \
           noteOn:\n    \
             fields:\n      \
               channel: { type: uint8, range: [1, 16] }\n";

    /// schema validator が違反として検出する events.yaml フィクスチャ。
    /// `range:` の min > max を含む。
    const EVENTS_YAML_INVALID_RANGE: &str = "schema_version: 1\n\
         events:\n  \
           noteOn:\n    \
             fields:\n      \
               channel: { type: uint8, range: [16, 1] }\n";

    /// runtime feature-availability check が reject する events.yaml
    /// フィクスチャ。`tier: streamed` を 1 件含む。
    const EVENTS_YAML_STREAMED_OSCBLOB: &str = "schema_version: 1\n\
         events:\n  \
           oscBlob:\n    \
             tier: streamed\n    \
             fields:\n      \
               payload: { type: bytes, max_length: 1024 }\n";

    /// loader 段階の YAML パースで失敗する events.yaml フィクスチャ。
    /// 末尾の `[` が閉じておらず `serde_yaml_ng::from_str` が Parse error を返す。
    const EVENTS_YAML_MALFORMED: &str = "schema_version: [\n";

    /// `transform` フィールドを欠いた profile YAML フィクスチャ。
    /// `ProfileLoadError::Parse` 経路の回帰用。
    const PROFILE_YAML_MISSING_TRANSFORM: &str = "inputs:\n  - adapter: a.yaml\n    connection: { driver: midi }\noutputs:\n  - adapter: b.yaml\n    connection: { driver: osc }\n";

    fn run_args(profile_path: &Path, app_data_dir: &Path) -> Vec<String> {
        vec![
            "midori".to_owned(),
            "--app-data-dir".to_owned(),
            app_data_dir.to_str().expect("utf-8").to_owned(),
            "run".to_owned(),
            profile_path.to_str().expect("utf-8").to_owned(),
        ]
    }

    #[test]
    fn it_should_pass_startup_when_profile_drivers_have_valid_events_yaml() {
        let app = setup_app_data_dir(&[("midi", EVENTS_YAML_VALID_NOTEON)]);
        let profile = write_tmp_profile("happy", &[("input", "midi"), ("output", "midi")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let result = super::dispatch(&cli);
        assert!(result.is_ok(), "valid events.yaml should pass: {result:?}");
    }

    #[test]
    fn it_should_fail_startup_when_driver_events_yaml_violates_schema() {
        let app = setup_app_data_dir(&[("midi", EVENTS_YAML_INVALID_RANGE)]);
        let profile = write_tmp_profile("invalid", &[("input", "midi"), ("output", "midi")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let err = super::dispatch(&cli).expect_err("schema violation must fail");
        assert!(
            matches!(
                err,
                CliError::StartupCheck {
                    source: midori_runtime::events_pipeline::StartupCheckError::Validate { .. }
                }
            ),
            "expected StartupCheck::Validate, got {err:?}"
        );
    }

    #[test]
    fn it_should_warn_and_continue_when_driver_events_yaml_is_missing() {
        // driver 自体は plugin manifest からは解決できる（driver.yaml はある）が、
        // 隣接する events.yaml だけ無い → Missing 扱いで warn + 起動継続。
        let app = tempfile::Builder::new()
            .prefix("midori-profile-test-app-missing-")
            .tempdir()
            .expect("tempdir");
        let plugin_root = app.path().join("plugins").join("midi-plugin");
        let dot_midori = plugin_root.join(".midori");
        std::fs::create_dir_all(&dot_midori).expect("mkdir");
        std::fs::write(
            dot_midori.join("plugin.yaml"),
            "name: midi-plugin\ndrivers:\n  - driver: ../drivers/midi/driver.yaml\n",
        )
        .expect("write plugin.yaml");
        let driver_dir = plugin_root.join("drivers").join("midi");
        std::fs::create_dir_all(&driver_dir).expect("mkdir driver");
        std::fs::write(
            driver_dir.join("driver.yaml"),
            "name: midi\nmodality: midi\n",
        )
        .expect("write driver.yaml");
        // events.yaml は **置かない**

        let profile = write_tmp_profile("missing", &[("input", "midi"), ("output", "midi")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let result = super::dispatch(&cli);
        assert!(
            result.is_ok(),
            "missing events.yaml should be a warning: {result:?}"
        );
    }

    #[test]
    fn it_should_fail_when_profile_driver_is_not_installed_as_plugin() {
        // plugin が 1 件も無い → profile の driver 名は plugin manifest から
        // 解決できず DriverNotInstalled。spec 上「未インストール driver は
        // 起動時エラー」(`design/09-plugin.md`)。
        let app = setup_app_data_dir(&[]);
        let profile = write_tmp_profile("absent-plugin", &[("input", "midi"), ("output", "midi")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let err = super::dispatch(&cli).expect_err("missing plugin must fail");
        assert!(
            matches!(err, CliError::DriverNotInstalled { ref driver_name } if driver_name == "midi"),
            "expected DriverNotInstalled, got {err:?}"
        );
    }

    #[test]
    fn it_should_fail_startup_when_driver_declares_streamed_tier() {
        let app = setup_app_data_dir(&[("osc", EVENTS_YAML_STREAMED_OSCBLOB)]);
        let profile = write_tmp_profile("streamed", &[("input", "osc"), ("output", "osc")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let err = super::dispatch(&cli).expect_err("streamed must be rejected at startup");
        assert!(
            matches!(
                err,
                CliError::StartupCheck {
                    source:
                        midori_runtime::events_pipeline::StartupCheckError::FeatureUnavailable { .. }
                }
            ),
            "expected StartupCheck::FeatureUnavailable, got {err:?}"
        );
    }

    #[test]
    fn it_should_fail_startup_when_driver_events_yaml_cannot_be_loaded() {
        let app = setup_app_data_dir(&[("midi", EVENTS_YAML_MALFORMED)]);
        let profile = write_tmp_profile("load-error", &[("input", "midi"), ("output", "midi")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let err = super::dispatch(&cli).expect_err("malformed YAML must fail");
        assert!(
            matches!(
                err,
                CliError::StartupCheck {
                    source: midori_runtime::events_pipeline::StartupCheckError::Load { .. }
                }
            ),
            "expected StartupCheck::Load, got {err:?}"
        );
    }

    #[test]
    fn it_should_iterate_through_input_and_output_drivers_and_fail_on_violating_one() {
        // 1 件目 (input=midi) は valid、2 件目 (output=osc) で schema 違反
        let app = setup_app_data_dir(&[
            ("midi", EVENTS_YAML_VALID_NOTEON),
            ("osc", EVENTS_YAML_INVALID_RANGE),
        ]);
        let profile = write_tmp_profile("multi-fail", &[("input", "midi"), ("output", "osc")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let err = super::dispatch(&cli).expect_err("second entry violates schema");
        assert!(
            matches!(
                err,
                CliError::StartupCheck {
                    source: midori_runtime::events_pipeline::StartupCheckError::Validate { .. }
                }
            ),
            "expected StartupCheck::Validate, got {err:?}"
        );
    }

    #[test]
    fn it_should_continue_through_missing_events_yaml_when_others_are_valid() {
        // input (`midi`) は events.yaml 配置済。output (`absent`) は plugin /
        // driver.yaml は配置するが events.yaml だけ欠落 → Missing 扱いで継続。
        let app = setup_app_data_dir(&[("midi", EVENTS_YAML_VALID_NOTEON)]);
        // `absent` driver を plugin manifest からは解決できるよう用意するが、
        // events.yaml だけ追加で削除する。
        let absent_plugin = app.path().join("plugins").join("absent-plugin");
        let dot_midori = absent_plugin.join(".midori");
        std::fs::create_dir_all(&dot_midori).expect("mkdir");
        std::fs::write(
            dot_midori.join("plugin.yaml"),
            "name: absent-plugin\ndrivers:\n  - driver: ../drivers/absent/driver.yaml\n",
        )
        .expect("write plugin.yaml");
        let driver_dir = absent_plugin.join("drivers").join("absent");
        std::fs::create_dir_all(&driver_dir).expect("mkdir");
        std::fs::write(
            driver_dir.join("driver.yaml"),
            "name: absent\nmodality: absent\n",
        )
        .expect("write driver.yaml");
        // events.yaml は意図的に書かない

        let profile = write_tmp_profile("multi-mixed", &[("input", "midi"), ("output", "absent")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let result = super::dispatch(&cli);
        assert!(
            result.is_ok(),
            "missing events.yaml should warn + continue: {result:?}"
        );
    }

    #[test]
    fn it_should_dedupe_drivers_appearing_in_both_input_and_output() {
        // 同一 driver が input と output 双方に出現するケース。events.yaml は
        // 1 度だけ load される（dedupe）想定で、無効な 2 度目 load が走らない
        // ことは少なくとも「正常通過」で観測できる。
        let app = setup_app_data_dir(&[("midi", EVENTS_YAML_VALID_NOTEON)]);
        let profile = write_tmp_profile("dedupe", &[("input", "midi"), ("output", "midi")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let result = super::dispatch(&cli);
        assert!(result.is_ok(), "dedupe path: {result:?}");
    }

    #[test]
    fn it_should_fail_when_profile_yaml_is_invalid() {
        // 必須フィールド (transform) を欠いた profile YAML は serde 段階で
        // 弾かれ、ProfileLoadError::Parse として CliError::LoadProfile に
        // 包まれる。inner variant も pin して Io / Invalid との取り違えを防ぐ。
        let app = setup_app_data_dir(&[]);
        let mut bad = tempfile::Builder::new()
            .prefix("midori-profile-test-bad-")
            .suffix(".yaml")
            .tempfile()
            .expect("tempfile");
        std::io::Write::write_all(&mut bad, PROFILE_YAML_MISSING_TRANSFORM.as_bytes())
            .expect("write");

        let cli = Cli::try_parse_from(run_args(bad.path(), app.path())).expect("parse");
        let err = super::dispatch(&cli).expect_err("missing transform");
        assert!(
            matches!(
                err,
                CliError::LoadProfile {
                    source: crate::profile::ProfileLoadError::Parse { .. }
                }
            ),
            "expected LoadProfile::Parse, got {err:?}"
        );
    }

    #[test]
    fn it_should_render_startup_check_error_display_with_driver_and_violation_context() {
        let app = setup_app_data_dir(&[("midi", EVENTS_YAML_INVALID_RANGE)]);
        let profile = write_tmp_profile("display", &[("input", "midi"), ("output", "midi")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let err = super::dispatch(&cli).expect_err("schema violation");
        let rendered = err.to_string();
        assert!(rendered.contains("midi"), "got: {rendered}");
        assert!(rendered.contains("schema 違反"), "got: {rendered}");
        assert!(!rendered.ends_with('\n'), "got: {rendered:?}");
    }

    #[test]
    fn it_should_render_startup_check_error_display_with_streamed_feature_reason() {
        let app = setup_app_data_dir(&[("osc", EVENTS_YAML_STREAMED_OSCBLOB)]);
        let profile = write_tmp_profile("display-streamed", &[("input", "osc"), ("output", "osc")]);
        let cli = Cli::try_parse_from(run_args(profile.path(), app.path())).expect("parse");
        let err = super::dispatch(&cli).expect_err("streamed");
        let rendered = err.to_string();
        assert!(rendered.contains("osc"), "got: {rendered}");
        assert!(rendered.contains("streamed"), "got: {rendered}");
        // 内側 FeatureUnavailable.reason の文言まで chain して観測できることを
        // 担保する（feature_check.rs::STREAMED_REASON 由来）。
        assert!(
            rendered.contains("not implemented"),
            "Display should include the inner reason phrase, got: {rendered}"
        );
        assert!(!rendered.ends_with('\n'), "got: {rendered:?}");
    }
}
