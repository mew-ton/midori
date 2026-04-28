//! events.yaml の YAML パース I/O とロード経路。

use std::fs;
use std::path::{Path, PathBuf};

use super::types::EventsSchema;

/// Result of loading a driver's `events.yaml`.
#[derive(Debug)]
pub enum LoadOutcome {
    /// File was found and parsed successfully (still needs `validate`).
    Loaded(EventsSchema),
    /// File does not exist. Per spec, the caller may treat this as
    /// "schema-undeclared mode" with a warning, or drop all events.
    Missing,
}

/// Errors raised while loading `events.yaml`.
#[derive(Debug)]
pub enum LoadError {
    /// I/O error reading the YAML file.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// YAML parse / deserialization failure.
    Parse {
        path: PathBuf,
        source: serde_yml::Error,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    f,
                    "failed to read events.yaml at {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, source } => write!(
                f,
                "failed to parse events.yaml at {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

/// Load `events.yaml` from `path`. Returns `LoadOutcome::Missing` when the
/// file does not exist (per spec the caller decides drop-all vs warning).
///
/// **Note**: 本関数は YAML パースのみを行い、schema バリデーション
/// (`validate`) は実行しない。`LoadOutcome::Loaded` を受け取った caller は
/// **必ず** `validate(&schema)` を続けて呼び出して schema 違反を検出する
/// 責任を負う（load と validate を分離してあるのは、load 段階の I/O 失敗と
/// validate 段階の意味論違反を別系統で報告するため）。
pub fn load_from_path(path: &Path) -> Result<LoadOutcome, LoadError> {
    let yaml = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LoadOutcome::Missing);
        }
        Err(source) => {
            return Err(LoadError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let schema: EventsSchema = serde_yml::from_str(&yaml).map_err(|source| LoadError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(LoadOutcome::Loaded(schema))
}

/// Resolve `events.yaml` path from the directory containing `driver.yaml`.
///
/// Per spec: `events.yaml` lives next to `driver.yaml`.
#[must_use]
pub fn resolve_events_yaml_path(driver_yaml_dir: &Path) -> PathBuf {
    driver_yaml_dir.join("events.yaml")
}
