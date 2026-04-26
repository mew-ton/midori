use std::path::PathBuf;

#[derive(Debug)]
pub enum CliError {
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
