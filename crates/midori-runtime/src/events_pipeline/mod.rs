//! Inline payload を Layer 2 binding 入口まで運ぶ runtime パイプライン層。
//!
//! 役割は 3 段:
//!
//! 1. msgpack バイト列の decode（[`decode`] サブモジュール）
//! 2. 1 イベント単位の events.yaml schema 照合（[`runtime_check`] サブモジュール）
//! 3. 照合通過イベントを Layer 2 binding 側へ手渡す（本モジュールの [`EventSink`]）
//!
//! Layer 2 binding 本体は MEW-50 のスコープ外。本層は **stub I/F** として
//! `EventSink` trait と最小実装 [`LoggingSink`] を提供し、後続の binding 実装
//! で差し替える前提。
//!
//! 不正イベント（decode 失敗 / schema 違反 / events.yaml 未宣言 driver）は
//! [`process_inline_payload`] が Error ログを出して drop する。パイプライン
//! 全体は止めず、別 driver の処理は継続する。
//!
//! 構成:
//!
//! - [`decode`]: msgpack → [`DecodedPayload`]
//! - [`runtime_check`]: [`DecodedPayload`] × `EventsSchema` → [`ValidatedEvent`]
//! - 本ファイル: [`EventSink`] / [`LoggingSink`] / [`process_inline_payload`]
//! - `tests`: 統合テスト

// 本 module の公開 API は Bridge 起動 pipeline からまだ呼び出されておらず
// （後続 subtask で接続予定）、binary crate 内の dead_code / unused_imports
// 検出に引っかかるため module 全体で抑制する。実体は単体テストで網羅している。
#![allow(dead_code, unused_imports)]

mod decode;
mod runtime_check;

pub use decode::{decode_event, DecodeError, DecodedPayload, FieldValue};
pub use runtime_check::{check_event, RuntimeCheckError, ValidatedEvent};

use crate::events_schema::EventsSchema;

/// Layer 2 binding が schema 照合通過後のイベントを受け取る接点。
///
/// 本 trait は MEW-50 段階の **stub**。実 binding 経路の I/F が固まり次第
/// 差し替える（trait のままにするか、channel ベースへ移すかは後続 Issue で決定）。
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
