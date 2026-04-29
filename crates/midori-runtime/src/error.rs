use std::path::PathBuf;

use crate::events_pipeline::StartupCheckError;

#[derive(Debug)]
pub enum CliError {
    ReadProfile {
        path: PathBuf,
        source: std::io::Error,
    },
    /// `--driver-events` 引数の値が `<name>=<path>` の形式に従っていない。
    InvalidDriverEventsArg { raw: String },
    /// 起動時の events.yaml チェックで Bridge が継続できない違反を検出した。
    StartupCheck { source: StartupCheckError },
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
            Self::InvalidDriverEventsArg { raw } => write!(
                f,
                "--driver-events の値が `<name>=<path>` 形式ではありません: `{raw}`"
            ),
            Self::StartupCheck { source } => write!(f, "起動時チェックに失敗しました: {source}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadProfile { source, .. } => Some(source),
            Self::StartupCheck { source } => Some(source),
            Self::InvalidDriverEventsArg { .. } => None,
        }
    }
}
