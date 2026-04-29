mod error;
mod events_pipeline;
mod events_schema;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use crate::error::CliError;
use crate::events_pipeline::{check_driver_schema, DriverSchemaOutcome};

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

        /// driver の events.yaml を `<name>=<path>` 形式で直接指定する。
        /// profile YAML resolver 完成までの暫定経路で、CLI help では非露出。
        /// 複数回指定可。
        #[arg(long = "driver-events", value_name = "NAME=PATH", hide = true)]
        driver_events: Vec<String>,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("midori: {err}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: &Cli) -> Result<(), CliError> {
    match &cli.command {
        Command::Run {
            profile,
            driver_events,
        } => run(profile, driver_events),
    }
}

// プロファイル本体のパイプラインは後続の subtask で実装する。
// 本関数では (a) プロファイル YAML が読めること、(b) 暫定経路で渡された
// `--driver-events` の events.yaml が起動時チェックを通過することのみ確認する。
fn run(profile_path: &Path, driver_events_args: &[String]) -> Result<(), CliError> {
    let _profile_yaml =
        std::fs::read_to_string(profile_path).map_err(|source| CliError::ReadProfile {
            path: profile_path.to_path_buf(),
            source,
        })?;

    for raw in driver_events_args {
        let (name, path) = parse_driver_events_arg(raw)?;
        match check_driver_schema(name, &path)
            .map_err(|source| CliError::StartupCheck { source })?
        {
            DriverSchemaOutcome::Loaded(_) => {
                eprintln!(
                    "midori: driver `{name}` の events.yaml ({}) のチェックが完了しました",
                    path.display()
                );
            }
            DriverSchemaOutcome::Missing => {
                // events.yaml が無い driver は spec 上「明示的な schema 未宣言モード」
                // として warning に留めて起動を継続する。
                eprintln!(
                    "midori: warning: driver `{name}` の events.yaml ({}) が見つかりませんでした。schema 未宣言モードで起動します",
                    path.display()
                );
            }
        }
    }

    Ok(())
}

/// `--driver-events <name>=<path>` 形式の引数を分解する。
///
/// shell からの引数渡しで前後に空白が混じるケース（例: `"midi=   "`）を
/// 防衛するため、`name` / `path` は `trim()` してから空判定する。
fn parse_driver_events_arg(raw: &str) -> Result<(&str, PathBuf), CliError> {
    let (name, path) = raw
        .split_once('=')
        .ok_or_else(|| CliError::InvalidDriverEventsArg {
            raw: raw.to_owned(),
        })?;
    let name = name.trim();
    let path = path.trim();
    if name.is_empty() || path.is_empty() {
        return Err(CliError::InvalidDriverEventsArg {
            raw: raw.to_owned(),
        });
    }
    Ok((name, PathBuf::from(path)))
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, LogFormat, LogLevel};
    use clap::{CommandFactory, Parser};
    use std::path::PathBuf;

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
            Command::Run {
                profile,
                driver_events,
            } => {
                assert_eq!(profile, PathBuf::from("/tmp/profile.yaml"));
                assert!(driver_events.is_empty());
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
    fn it_should_succeed_when_profile_file_exists() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("midori-runtime-test-{}.yaml", std::process::id()));
        std::fs::write(&path, "name: test\n").expect("write tmp profile");

        let cli = Cli::try_parse_from(["midori", "run", path.to_str().expect("tmp path is utf-8")])
            .expect("parse");
        let result = super::dispatch(&cli);

        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok(), "existing profile should load");
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
    // --driver-events startup chain
    // --------------------------------------------------------------

    use crate::error::CliError;

    /// 必要最小の profile YAML をテンポラリ生成し、その path を返す。
    fn write_tmp_profile(tag: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new()
            .prefix(&format!("midori-mew54-{tag}-"))
            .suffix(".yaml")
            .tempfile()
            .expect("tempfile");
        std::io::Write::write_all(&mut file, b"name: test\n").expect("write profile");
        file
    }

    /// 任意の events.yaml 文字列を temp ファイルに書き出す。
    fn write_tmp_events_yaml(tag: &str, body: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new()
            .prefix(&format!("midori-mew54-events-{tag}-"))
            .suffix(".yaml")
            .tempfile()
            .expect("tempfile");
        std::io::Write::write_all(&mut file, body.as_bytes()).expect("write events.yaml");
        file
    }

    #[test]
    fn it_should_pass_startup_when_driver_events_yaml_is_valid() {
        let profile = write_tmp_profile("happy");
        let events = write_tmp_events_yaml(
            "happy",
            "schema_version: 1\n\
             events:\n  \
               noteOn:\n    \
                 fields:\n      \
                   channel: { type: uint8, range: [1, 16] }\n",
        );
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            &format!("midi={}", events.path().display()),
        ])
        .expect("parse");

        let result = super::dispatch(&cli);

        assert!(result.is_ok(), "valid events.yaml should pass: {result:?}");
    }

    #[test]
    fn it_should_fail_startup_when_driver_events_yaml_violates_schema() {
        // `range:` の min > max は validator が違反として検出する。
        let profile = write_tmp_profile("invalid");
        let events = write_tmp_events_yaml(
            "invalid",
            "schema_version: 1\n\
             events:\n  \
               noteOn:\n    \
                 fields:\n      \
                   channel: { type: uint8, range: [16, 1] }\n",
        );
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            &format!("midi={}", events.path().display()),
        ])
        .expect("parse");

        let err = super::dispatch(&cli).expect_err("schema violation must fail");
        assert!(
            matches!(
                err,
                CliError::StartupCheck {
                    source: crate::events_pipeline::StartupCheckError::Validate { .. }
                }
            ),
            "expected StartupCheck::Validate, got {err:?}"
        );
    }

    #[test]
    fn it_should_warn_and_continue_when_driver_events_yaml_is_missing() {
        let profile = write_tmp_profile("missing");
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            "midi=/nonexistent/midori-mew54/events.yaml",
        ])
        .expect("parse");

        let result = super::dispatch(&cli);

        assert!(
            result.is_ok(),
            "missing events.yaml should be a warning, not a startup failure: {result:?}"
        );
    }

    #[test]
    fn it_should_fail_startup_when_driver_declares_streamed_tier() {
        let profile = write_tmp_profile("streamed");
        let events = write_tmp_events_yaml(
            "streamed",
            "schema_version: 1\n\
             events:\n  \
               oscBlob:\n    \
                 tier: streamed\n    \
                 fields:\n      \
                   payload: { type: bytes, max_length: 1024 }\n",
        );
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            &format!("osc={}", events.path().display()),
        ])
        .expect("parse");

        let err = super::dispatch(&cli).expect_err("streamed must be rejected at startup");
        assert!(
            matches!(
                err,
                CliError::StartupCheck {
                    source: crate::events_pipeline::StartupCheckError::FeatureUnavailable { .. }
                }
            ),
            "expected StartupCheck::FeatureUnavailable, got {err:?}"
        );
    }

    #[test]
    fn it_should_reject_malformed_driver_events_argument() {
        let profile = write_tmp_profile("malformed");
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            "no-equals-sign",
        ])
        .expect("parse");

        let err = super::dispatch(&cli).expect_err("malformed arg must fail");
        assert!(
            matches!(err, CliError::InvalidDriverEventsArg { .. }),
            "expected InvalidDriverEventsArg, got {err:?}"
        );
    }

    #[test]
    fn it_should_reject_driver_events_argument_with_empty_name() {
        let profile = write_tmp_profile("empty-name");
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            "=/tmp/events.yaml",
        ])
        .expect("parse");

        let err = super::dispatch(&cli).expect_err("empty name must fail");
        assert!(
            matches!(err, CliError::InvalidDriverEventsArg { .. }),
            "expected InvalidDriverEventsArg, got {err:?}"
        );
    }

    #[test]
    fn it_should_reject_driver_events_argument_with_empty_path() {
        let profile = write_tmp_profile("empty-path");
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            "midi=",
        ])
        .expect("parse");

        let err = super::dispatch(&cli).expect_err("empty path must fail");
        assert!(
            matches!(err, CliError::InvalidDriverEventsArg { .. }),
            "expected InvalidDriverEventsArg, got {err:?}"
        );
    }

    #[test]
    fn it_should_reject_driver_events_argument_with_whitespace_only_name() {
        let profile = write_tmp_profile("ws-name");
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            "   =/tmp/events.yaml",
        ])
        .expect("parse");

        let err = super::dispatch(&cli).expect_err("whitespace-only name must fail");
        assert!(
            matches!(err, CliError::InvalidDriverEventsArg { .. }),
            "expected InvalidDriverEventsArg, got {err:?}"
        );
    }

    #[test]
    fn it_should_reject_driver_events_argument_with_whitespace_only_path() {
        let profile = write_tmp_profile("ws-path");
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            "midi=   ",
        ])
        .expect("parse");

        let err = super::dispatch(&cli).expect_err("whitespace-only path must fail");
        assert!(
            matches!(err, CliError::InvalidDriverEventsArg { .. }),
            "expected InvalidDriverEventsArg, got {err:?}"
        );
    }

    #[test]
    fn it_should_render_startup_check_error_display_with_driver_and_violation_context() {
        // schema 違反をわざと作って、Display 文字列に driver 名と違反内容が
        // 両方含まれることを担保する。trailing newline がないことも検査する。
        let profile = write_tmp_profile("display");
        let events = write_tmp_events_yaml(
            "display",
            "schema_version: 1\n\
             events:\n  \
               noteOn:\n    \
                 fields:\n      \
                   channel: { type: uint8, range: [16, 1] }\n",
        );
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            &format!("midi={}", events.path().display()),
        ])
        .expect("parse");

        let err = super::dispatch(&cli).expect_err("schema violation");
        let rendered = err.to_string();
        assert!(
            rendered.contains("midi"),
            "Display should mention the driver name, got: {rendered}"
        );
        assert!(
            rendered.contains("schema 違反"),
            "Display should mention the violation kind, got: {rendered}"
        );
        assert!(
            !rendered.ends_with('\n'),
            "Display must not end with a trailing newline, got: {rendered:?}"
        );
    }

    #[test]
    fn it_should_render_startup_check_error_display_with_streamed_feature_reason() {
        let profile = write_tmp_profile("display-streamed");
        let events = write_tmp_events_yaml(
            "display-streamed",
            "schema_version: 1\n\
             events:\n  \
               oscBlob:\n    \
                 tier: streamed\n    \
                 fields:\n      \
                   payload: { type: bytes, max_length: 1024 }\n",
        );
        let cli = Cli::try_parse_from([
            "midori",
            "run",
            profile.path().to_str().expect("profile utf-8"),
            "--driver-events",
            &format!("osc={}", events.path().display()),
        ])
        .expect("parse");

        let err = super::dispatch(&cli).expect_err("streamed");
        let rendered = err.to_string();
        assert!(
            rendered.contains("osc"),
            "Display should mention the driver name, got: {rendered}"
        );
        assert!(
            rendered.contains("streamed"),
            "Display should mention the unsupported feature, got: {rendered}"
        );
        assert!(
            !rendered.ends_with('\n'),
            "Display must not end with a trailing newline, got: {rendered:?}"
        );
    }
}
