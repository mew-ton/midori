//! events.yaml schema loader and validator for Bridge.
//!
//! Parses driver-provided `events.yaml` per the spec in
//! `design/16-driver-events-schema.md` and validates the structure at Bridge
//! startup. Schema violations are reported as startup errors (callers fail
//! fast) rather than runtime drops.
//!
//! Out of scope here: msgpack decode of raw events, and Layer 2 binding wiring.
//! Those live in sibling modules.
//!
//! 構成（責務別 sub-module + テスト分離）:
//!
//! - [`types`]:         events.yaml の Rust 表現と serde 周辺
//! - [`loader`]:        YAML パース I/O とエラー
//! - [`validator`]:     schema 違反検出ルール（構文・意味論レベル）
//! - [`feature_check`]: Bridge runtime が現状サポートしていない宣言の reject
//! - `tests`:           sub-module すべての単体テスト

// `events_pipeline::startup` 経由で `load_from_path` / `validate` /
// `check_runtime_features` は実 caller を持つ。一方で `EventsSchema` /
// `EventDef` / `FieldSpec` などの内部 field は Layer 2 binding 連携が
// 完成するまで読み出されない。後続 subtask で本格 caller が付くため、
// 当面は module 全体で抑制を残す（実体は単体テストで網羅している）。
#![allow(dead_code, unused_imports)]

mod feature_check;
mod loader;
mod types;
mod validator;

pub use feature_check::{check_runtime_features, FeatureUnavailable};
pub use loader::{load_from_path, resolve_events_yaml_path, LoadError, LoadOutcome};
pub use types::{EventDef, EventsSchema, FieldSpec, FieldType, RangeBound, Tier};
// 同 crate 内の `events_pipeline::runtime_check` も範囲比較で使うため
// crate スコープで再エクスポートする（外部 API には出さない）。
pub(crate) use types::yaml_to_f64;
pub use validator::{validate, ValidationError};

#[cfg(test)]
mod tests;
