use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

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
        Command::Run { profile } => run(profile),
    }
}

// パイプライン本体は MEW-23 以降で実装する。
// ここではプロファイルが読み込めることだけを確認する骨格を置く。
fn run(profile_path: &Path) -> Result<(), CliError> {
    let _profile_yaml =
        std::fs::read_to_string(profile_path).map_err(|source| CliError::ReadProfile {
            path: profile_path.to_path_buf(),
            source,
        })?;
    Ok(())
}

#[derive(Debug)]
enum CliError {
    ReadProfile {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadProfile { path, source } => {
                write!(
                    f,
                    "プロファイルの読み込みに失敗しました ({}): {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadProfile { source, .. } => Some(source),
        }
    }
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
}
