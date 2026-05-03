//! Inline payload を Layer 2 binding 入口まで運ぶ runtime パイプライン層。
//!
//! 役割は 4 つ:
//!
//! 1. Bridge 起動時に各 driver の events.yaml を整合性チェック
//!    （[`startup`] サブモジュール、main から駆動）
//! 2. msgpack バイト列の decode（[`decode`] サブモジュール）
//! 3. 1 イベント単位の events.yaml schema 照合（[`runtime_check`] サブモジュール）
//! 4. 照合通過イベントを Layer 2 binding 側へ手渡す（本モジュールの [`EventSink`]）
//!
//! Layer 2 binding 本体はまだ未実装で、本層は **stub I/F** として `EventSink`
//! trait と最小実装 [`LoggingSink`] を提供する。実 binding 経路の I/F が
//! 固まり次第差し替える。
//!
//! 不正イベント（decode 失敗 / schema 違反 / events.yaml 未宣言 driver）は
//! [`process_inline_payload`] が Error ログを出して drop する。パイプライン
//! 全体は止めず、別 driver の処理は継続する。
//!
//! 構成:
//!
//! - [`startup`]: events.yaml の起動時整合性チェック（[`check_driver_schema`]）
//! - [`decode`]: msgpack → [`DecodedPayload`]
//! - [`runtime_check`]: [`DecodedPayload`] × `EventsSchema` → [`ValidatedEvent`]
//! - 本ファイル: [`EventSink`] / [`LoggingSink`] / [`process_inline_payload`]
//! - `tests`: 統合テスト

// `decode` / `runtime_check` は `ring_ingest` 経由の実 caller が付いた
// ので dead_code 抑制を外す。`startup` は main から駆動済み。
mod decode;
mod runtime_check;
mod startup;

pub use decode::{decode_event, DecodeError, DecodedPayload, FieldValue};
pub use runtime_check::{check_event, RuntimeCheckError, ValidatedEvent};
pub use startup::{check_driver_schema, DriverSchemaOutcome, StartupCheckError};

use crate::events_schema::EventsSchema;

/// Layer 2 binding が schema 照合通過後のイベントを受け取る接点。
///
/// 本 trait は **stub**。実 binding 経路の I/F が固まり次第差し替える
/// （trait のままにするか、channel ベースへ移すかは未定）。
pub trait EventSink {
    fn dispatch(&mut self, event: ValidatedEvent);
}

/// stderr に最小情報だけ書き出す `EventSink` の素朴実装。trace 用途。
pub struct LoggingSink;

impl EventSink for LoggingSink {
    fn dispatch(&mut self, event: ValidatedEvent) {
        eprintln!(
            "midori: layer2-stub recv driver=`{}` type=`{}` fields={}",
            event.driver_name,
            event.event_type,
            event.fields.len()
        );
    }
}

/// Decode → schema check → dispatch の 3 段を直列に実行する。
///
/// `schema` が `None` のとき、つまり driver の events.yaml がロード不能
/// だったケースは 1 件単位で Error ログを出し drop する（schema が無いと
/// 値検証ができないため、wire format が偶然正しくても受理しない）。caller
/// が `LoadOutcome::Missing` を warning として扱いつつパイプラインを継続
/// したい場合も、本関数は payload を sink へ流さない。
pub fn process_inline_payload(
    driver_name: &str,
    schema: Option<&EventsSchema>,
    payload: &[u8],
    sink: &mut dyn EventSink,
) {
    match try_process(driver_name, schema, payload) {
        Ok(event) => sink.dispatch(event),
        Err(err) => eprintln!("midori: drop event from driver `{driver_name}`: {err}"),
    }
}

/// `process_inline_payload` の純関数版。テストでログ捕捉に頼らず
/// 失敗種別をパターンマッチで検査するために露出する。
pub(crate) fn try_process(
    driver_name: &str,
    schema: Option<&EventsSchema>,
    payload: &[u8],
) -> Result<ValidatedEvent, ProcessError> {
    let decoded = decode_event(payload).map_err(ProcessError::Decode)?;
    let schema = schema.ok_or(ProcessError::SchemaMissing)?;
    let event = check_event(driver_name, schema, decoded).map_err(ProcessError::Check)?;
    Ok(event)
}

/// パイプラインの 3 段が出すエラーを集約した単一型（テスト・観測用）。
#[derive(Debug)]
pub(crate) enum ProcessError {
    Decode(DecodeError),
    SchemaMissing,
    Check(RuntimeCheckError),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(source) => write!(f, "msgpack decode failed: {source}"),
            Self::SchemaMissing => f.write_str("driver has no events.yaml schema loaded"),
            Self::Check(source) => write!(f, "schema check failed: {source}"),
        }
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(source) => Some(source),
            Self::Check(source) => Some(source),
            Self::SchemaMissing => None,
        }
    }
}

#[cfg(test)]
mod tests;
