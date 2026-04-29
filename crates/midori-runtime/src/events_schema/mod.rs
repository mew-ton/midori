//! events.yaml schema loader and validator for Bridge.
//!
//! Parses driver-provided `events.yaml` per the spec in
//! `design/16-driver-events-schema.md` and validates the structure at Bridge
//! startup. Schema violations are reported as startup errors (callers fail
//! fast) rather than runtime drops.
//!
//! Out of scope here: msgpack decode of raw events, runtime feature-availability
//! check for `tier: streamed`, and Layer 2 binding wiring. Those live in
//! sibling modules.
//!
//! 構成（責務別 sub-module + テスト分離）:
//!
//! - [`types`]:     events.yaml の Rust 表現と serde 周辺
//! - [`loader`]:    YAML パース I/O とエラー
//! - [`validator`]: schema 違反検出ルール
//! - `tests`:       sub-module すべての単体テスト

// この module の公開 API は Bridge パイプラインからまだ呼び出されておらず
// （後続 subtask で接続予定）、binary crate 内の dead_code / unused_imports
// 検出に引っかかるため module 全体で抑制する。実体は単体テストで網羅している。
#![allow(dead_code, unused_imports)]

mod loader;
mod types;
mod validator;

pub use loader::{load_from_path, resolve_events_yaml_path, LoadError, LoadOutcome};
pub use types::{EventDef, EventsSchema, FieldSpec, FieldType, RangeBound, Tier};
pub use validator::{validate, ValidationError};

#[cfg(test)]
mod tests;
