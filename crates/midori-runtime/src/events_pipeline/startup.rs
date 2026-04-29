//! Bridge 起動時の events.yaml 整合性チェック経路。
//!
//! `load_from_path → validate → check_runtime_features` を 1 関数として束ね、
//! main.rs などの caller が「driver 名 + events.yaml path」のリストを渡せば
//! 起動エラー or warning の判定を返す薄いラッパーを提供する。
//!
//! 実 driver process の spawn / 実 SPSC ring 経由の ingest は本層のスコープ外。
//! profile YAML resolver も別 subtask の責務で、本層は path を直接受け取る。

use std::path::{Path, PathBuf};

use crate::events_schema::{
    check_runtime_features, load_from_path, validate, EventsSchema, FeatureUnavailable, LoadError,
    LoadOutcome, ValidationError,
};

/// 1 driver ぶんの起動時整合性チェック結果。
///
/// `Loaded` が運ぶ `EventsSchema` は本 subtask の caller では未参照だが、
/// 後続の Layer 2 binding 配線で必要になる正式な戻り値のため保持しておく。
#[derive(Debug)]
pub enum DriverSchemaOutcome {
    /// events.yaml がロードでき、validator / feature check を通過した。
    Loaded(#[allow(dead_code)] EventsSchema),
    /// events.yaml が見つからなかった。caller は warning を出して継続する。
    Missing,
}

/// 起動時 events.yaml チェックの失敗種別。
#[derive(Debug)]
pub enum StartupCheckError {
    /// YAML の I/O / parse 失敗。
    Load {
        driver_name: String,
        path: PathBuf,
        source: LoadError,
    },
    /// schema 自身の整合性違反（validator が検出）。
    Validate {
        driver_name: String,
        path: PathBuf,
        violations: Vec<ValidationError>,
    },
    /// runtime が未対応な機能宣言（streamed tier 等）。
    FeatureUnavailable {
        driver_name: String,
        path: PathBuf,
        reasons: Vec<FeatureUnavailable>,
    },
}

impl std::fmt::Display for StartupCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load {
                driver_name,
                path,
                source,
            } => write!(
                f,
                "driver `{driver_name}` の events.yaml ({}) のロードに失敗しました: {source}",
                path.display()
            ),
            Self::Validate {
                driver_name,
                path,
                violations,
            } => {
                write!(
                    f,
                    "driver `{driver_name}` の events.yaml ({}) が schema 違反:",
                    path.display()
                )?;
                for v in violations {
                    write!(f, "\n  - {v}")?;
                }
                Ok(())
            }
            Self::FeatureUnavailable {
                driver_name,
                path,
                reasons,
            } => {
                write!(
                    f,
                    "driver `{driver_name}` の events.yaml ({}) が runtime 未対応な機能を宣言:",
                    path.display()
                )?;
                for r in reasons {
                    write!(f, "\n  - {r}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for StartupCheckError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Load { source, .. } => Some(source),
            Self::Validate { .. } | Self::FeatureUnavailable { .. } => None,
        }
    }
}

/// 1 driver ぶんの events.yaml を起動時整合性チェックする。
///
/// `path` は events.yaml そのものへの絶対 / 相対 path（`resolve_events_yaml_path`
/// の責務は caller が持つ）。Missing と検査通過は `Ok` で区別し、それ以外の
/// 失敗は `StartupCheckError` で返す。
pub fn check_driver_schema(
    driver_name: &str,
    path: &Path,
) -> Result<DriverSchemaOutcome, StartupCheckError> {
    let outcome = load_from_path(path).map_err(|source| StartupCheckError::Load {
        driver_name: driver_name.to_owned(),
        path: path.to_path_buf(),
        source,
    })?;

    let schema = match outcome {
        LoadOutcome::Missing => return Ok(DriverSchemaOutcome::Missing),
        LoadOutcome::Loaded(schema) => schema,
    };

    if let Err(violations) = validate(&schema) {
        return Err(StartupCheckError::Validate {
            driver_name: driver_name.to_owned(),
            path: path.to_path_buf(),
            violations,
        });
    }

    if let Err(reasons) = check_runtime_features(driver_name, &schema) {
        return Err(StartupCheckError::FeatureUnavailable {
            driver_name: driver_name.to_owned(),
            path: path.to_path_buf(),
            reasons,
        });
    }

    Ok(DriverSchemaOutcome::Loaded(schema))
}
