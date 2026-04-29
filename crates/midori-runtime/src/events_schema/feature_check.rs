//! Runtime feature-availability check for events.yaml.
//!
//! Schema validator が構文上は受理する宣言のうち、Bridge runtime のキャパビリティが
//! 未到達なものを起動時に reject する。streamed tier のように「将来実装予定だが
//! 現状未着手」な機能を **driver 作者が events.yaml に書ける** ようにしつつ、
//! 実利用は明示エラーで止める。

use super::types::{EventsSchema, Tier};

/// Streamed tier の説明文（エラーメッセージで利用）。
const STREAMED_FEATURE_LABEL: &str = "streamed";
const STREAMED_REASON: &str = "streamed tier is not implemented yet";

/// Runtime に未対応な機能宣言を一件単位で表す。
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FeatureUnavailable {
    pub driver_name: String,
    pub event_name: String,
    pub feature: &'static str,
    pub reason: &'static str,
}

impl std::fmt::Display for FeatureUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "driver `{}` event `{}` declares feature `{}` which the Bridge runtime cannot accept: {}",
            self.driver_name, self.event_name, self.feature, self.reason
        )
    }
}

impl std::error::Error for FeatureUnavailable {}

/// `schema` の宣言が現在の Bridge runtime で全て利用可能か検査する。
///
/// 1 driver につき 1 回、events.yaml ロード + `validate` 通過後に呼ぶ想定。
/// 全違反を集約して返すため、複数イベントで同じ未対応機能が宣言されている
/// 場合も一括で報告できる。
pub fn check_runtime_features(
    driver_name: &str,
    schema: &EventsSchema,
) -> Result<(), Vec<FeatureUnavailable>> {
    let mut errors = Vec::new();
    for (event_name, event) in &schema.events {
        if matches!(event.tier, Tier::Streamed) {
            errors.push(FeatureUnavailable {
                driver_name: driver_name.to_owned(),
                event_name: event_name.clone(),
                feature: STREAMED_FEATURE_LABEL,
                reason: STREAMED_REASON,
            });
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
