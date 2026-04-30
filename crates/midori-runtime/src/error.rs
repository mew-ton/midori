use crate::events_pipeline::StartupCheckError;
use crate::plugin_resolver::ResolveError;
use crate::profile::ProfileLoadError;

#[derive(Debug)]
pub enum CliError {
    /// プロファイル YAML のロード / パース / 意味論検証で失敗した。
    LoadProfile { source: ProfileLoadError },
    /// `--app-data-dir` 省略時に OS 標準のアプリデータディレクトリが
    /// 解決できなかった（環境変数 `HOME` / `APPDATA` などが未設定）。
    AppDataDirUnavailable,
    /// `<app-data-dir>/plugins/` 配下から driver manifest を解決できなかった。
    /// directory I/O 失敗または同名 driver の衝突。
    ResolvePlugins { source: ResolveError },
    /// プロファイルが要求する driver が plugin manifest から見つからなかった。
    DriverNotInstalled { driver_name: String },
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
            Self::ResolvePlugins { source } => {
                write!(f, "プラグイン解決に失敗しました: {source}")
            }
            Self::DriverNotInstalled { driver_name } => write!(
                f,
                "プロファイルが指定する driver `{driver_name}` を提供するプラグインが見つかりません。プラグインをインストールしてください"
            ),
            Self::StartupCheck { source } => write!(f, "起動時チェックに失敗しました: {source}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LoadProfile { source } => Some(source),
            Self::ResolvePlugins { source } => Some(source),
            Self::StartupCheck { source } => Some(source),
            Self::AppDataDirUnavailable | Self::DriverNotInstalled { .. } => None,
        }
    }
}
