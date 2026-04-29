use crate::events_pipeline::StartupCheckError;
use crate::profile::ProfileLoadError;

#[derive(Debug)]
pub enum CliError {
    /// プロファイル YAML のロード / パース / 意味論検証で失敗した。
    LoadProfile { source: ProfileLoadError },
    /// `--app-data-dir` 省略時に OS 標準のアプリデータディレクトリが
    /// 解決できなかった（環境変数 `HOME` / `APPDATA` などが未設定）。
    AppDataDirUnavailable,
    /// 起動時の events.yaml チェックで Bridge が継続できない違反を検出した。
    StartupCheck { source: StartupCheckError },
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadProfile { source } => write!(f, "{source}"),
            Self::AppDataDirUnavailable => f.write_str(
                "アプリデータディレクトリを自動解決できませんでした。--app-data-dir で明示してください",
            ),
            Self::StartupCheck { source } => write!(f, "起動時チェックに失敗しました: {source}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LoadProfile { source } => Some(source),
            Self::StartupCheck { source } => Some(source),
            Self::AppDataDirUnavailable => None,
        }
    }
}
