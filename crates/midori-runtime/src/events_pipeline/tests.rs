//! `events_pipeline` 全層の統合テスト。
//!
//! - `decode` 単独: msgpack バイト列の境界条件
//! - `runtime_check` 単独: schema 違反検出の網羅
//! - パイプライン (`process_inline_payload` / `try_process`): stub `EventSink`
//!   との結合、events.yaml 未ロード時の drop 挙動

use std::collections::BTreeMap;

use rmpv::Value;

use super::decode::{decode_event, DecodeError, FieldValue};
use super::runtime_check::{check_event, RuntimeCheckError, ValidatedEvent};
use super::{process_inline_payload, try_process, EventSink, ProcessError};
use crate::events_schema::EventsSchema;

// ============================================================
// fixtures
// ============================================================

/// テストで何度も使う MIDI 風の events.yaml を 1 箇所に固める。
/// `noteOn` / `realtime`（enum）/ `sysex`（bytes `max_length`）の 3 イベントで
/// runtime check の主要分岐を踏める。
fn midi_schema() -> EventsSchema {
    let yaml = r"
schema_version: 1
events:
  noteOn:
    fields:
      channel:  { type: uint8, range: [1, 16] }
      note:     { type: uint8, range: [0, 127] }
      velocity: { type: uint8, range: [0, 127] }
    binding_filter: [type, channel]
    note_field: note
  realtime:
    fields:
      message: { type: enum, values: [start, stop, continue, clock] }
    binding_filter: [type, message]
  sysex:
    fields:
      payload: { type: bytes, max_length: 8 }
    binding_filter: [type]
";
    serde_yml::from_str(yaml).expect("fixture parses")
}

fn encode_map(entries: &[(&str, Value)]) -> Vec<u8> {
    let value = Value::Map(
        entries
            .iter()
            .map(|(k, v)| (Value::String((*k).into()), v.clone()))
            .collect(),
    );
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value).expect("encode test fixture");
    buf
}

#[derive(Default)]
struct RecordingSink {
    events: Vec<ValidatedEvent>,
}

impl EventSink for RecordingSink {
    fn dispatch(&mut self, event: ValidatedEvent) {
        self.events.push(event);
    }
}

// ============================================================
// decode unit tests
// ============================================================

#[test]
fn it_should_decode_a_flat_msgpack_map_into_field_values() {
    let bytes = encode_map(&[
        ("type", Value::String("noteOn".into())),
        ("channel", Value::from(1u8)),
        ("note", Value::from(60u8)),
        ("velocity", Value::from(100u8)),
    ]);

    let decoded = decode_event(&bytes).expect("valid map decodes");

    assert_eq!(
        decoded.get("type"),
        Some(&FieldValue::String("noteOn".into()))
    );
    assert_eq!(decoded.get("channel"), Some(&FieldValue::UInt(1)));
    assert_eq!(decoded.get("note"), Some(&FieldValue::UInt(60)));
    assert_eq!(decoded.get("velocity"), Some(&FieldValue::UInt(100)));
}

#[test]
fn it_should_decode_negative_integers_as_signed() {
    let bytes = encode_map(&[
        ("type", Value::String("pitchBend".into())),
        ("value", Value::from(-4096i32)),
    ]);

    let decoded = decode_event(&bytes).expect("decodes");
    assert_eq!(decoded.get("value"), Some(&FieldValue::Int(-4096)));
}

#[test]
fn it_should_reject_non_map_top_level() {
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &Value::from(42u8)).expect("encode");

    let err = decode_event(&buf).expect_err("non-map must be rejected");
    assert!(
        matches!(err, DecodeError::NotAMap),
        "expected NotAMap, got {err:?}"
    );
}

#[test]
fn it_should_reject_truncated_msgpack() {
    let bytes = encode_map(&[("type", Value::String("noteOn".into()))]);
    let truncated = &bytes[..bytes.len() - 1];

    let err = decode_event(truncated).expect_err("truncated must fail to parse");
    assert!(
        matches!(err, DecodeError::Parse(_)),
        "expected Parse, got {err:?}"
    );
}

#[test]
fn it_should_reject_trailing_bytes_after_top_level_map() {
    let mut bytes = encode_map(&[("type", Value::String("noteOn".into()))]);
    bytes.push(0xc0); // extra msgpack nil

    let err = decode_event(&bytes).expect_err("trailing bytes must be rejected");
    assert!(
        matches!(err, DecodeError::TrailingBytes),
        "expected TrailingBytes, got {err:?}"
    );
}

#[test]
fn it_should_reject_non_string_map_keys() {
    // map with integer key 1 → "noteOn"
    let value = Value::Map(vec![(Value::from(1u8), Value::String("noteOn".into()))]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value).expect("encode");

    let err = decode_event(&buf).expect_err("non-string key must be rejected");
    assert!(
        matches!(err, DecodeError::NonStringKey),
        "expected NonStringKey, got {err:?}"
    );
}

#[test]
fn it_should_reject_nil_field_values() {
    let bytes = encode_map(&[
        ("type", Value::String("noteOn".into())),
        ("channel", Value::Nil),
    ]);

    let err = decode_event(&bytes).expect_err("nil must be rejected");
    assert!(
        matches!(err, DecodeError::UnsupportedValue { kind: "nil", .. }),
        "expected UnsupportedValue(nil), got {err:?}"
    );
}

#[test]
fn it_should_reject_msgpack_ext_field_values() {
    // ext type は events.yaml 語彙外。`Value::Ext(type_id, payload)` を 1 件
    // でも含む payload は decode 時点で reject されることを担保する。
    let bytes = encode_map(&[
        ("type", Value::String("noteOn".into())),
        ("custom", Value::Ext(42, vec![0xde, 0xad, 0xbe, 0xef])),
    ]);

    let err = decode_event(&bytes).expect_err("ext must be rejected");
    assert!(
        matches!(err, DecodeError::UnsupportedValue { kind: "ext", .. }),
        "expected UnsupportedValue(ext), got {err:?}"
    );
}

#[test]
fn it_should_reject_nested_maps_inside_a_field() {
    let nested = Value::Map(vec![(Value::String("x".into()), Value::from(1u8))]);
    let bytes = encode_map(&[("type", Value::String("noteOn".into())), ("inner", nested)]);

    let err = decode_event(&bytes).expect_err("nested map must be rejected");
    assert!(
        matches!(err, DecodeError::NestedMap { .. }),
        "expected NestedMap, got {err:?}"
    );
}

#[test]
fn it_should_reject_arrays_whose_elements_are_arrays() {
    let inner = Value::Array(vec![Value::from(1u8)]);
    let bytes = encode_map(&[
        ("type", Value::String("nested".into())),
        ("nums", Value::Array(vec![inner])),
    ]);

    let err = decode_event(&bytes).expect_err("array<array> must be rejected");
    assert!(
        matches!(err, DecodeError::NonScalarArrayElement { .. }),
        "expected NonScalarArrayElement, got {err:?}"
    );
}

// ============================================================
// runtime_check unit tests
// ============================================================

fn payload(entries: &[(&str, FieldValue)]) -> BTreeMap<String, FieldValue> {
    entries
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect()
}

#[test]
fn it_should_accept_a_valid_noteon_event_and_strip_the_type_discriminator() {
    let schema = midi_schema();
    let p = payload(&[
        ("type", FieldValue::String("noteOn".into())),
        ("channel", FieldValue::UInt(1)),
        ("note", FieldValue::UInt(60)),
        ("velocity", FieldValue::UInt(100)),
    ]);

    let event = check_event("midi", &schema, p).expect("valid event passes");

    assert_eq!(event.driver_name, "midi");
    assert_eq!(event.event_type, "noteOn");
    // `type` discriminator は ValidatedEvent.fields からは外れている
    assert!(!event.fields.contains_key("type"));
    assert_eq!(event.fields.len(), 3);
}

#[test]
fn it_should_reject_payloads_without_a_type_field() {
    let schema = midi_schema();
    let p = payload(&[("channel", FieldValue::UInt(1))]);

    let err = check_event("midi", &schema, p).expect_err("type missing");
    assert_eq!(err, RuntimeCheckError::MissingTypeField);
}

#[test]
fn it_should_reject_payloads_whose_type_is_not_a_string() {
    let schema = midi_schema();
    let p = payload(&[("type", FieldValue::UInt(42))]);

    let err = check_event("midi", &schema, p).expect_err("type must be string");
    assert_eq!(err, RuntimeCheckError::NonStringType);
}

#[test]
fn it_should_reject_unknown_event_types() {
    let schema = midi_schema();
    let p = payload(&[("type", FieldValue::String("unknownEvent".into()))]);

    let err = check_event("midi", &schema, p).expect_err("unknown type");
    assert_eq!(
        err,
        RuntimeCheckError::UnknownEventType {
            event_type: "unknownEvent".into()
        }
    );
}

#[test]
fn it_should_reject_when_a_required_field_is_missing() {
    let schema = midi_schema();
    let p = payload(&[
        ("type", FieldValue::String("noteOn".into())),
        ("channel", FieldValue::UInt(1)),
        ("note", FieldValue::UInt(60)),
        // velocity 抜け
    ]);

    let err = check_event("midi", &schema, p).expect_err("missing required");
    assert_eq!(
        err,
        RuntimeCheckError::RequiredFieldMissing {
            event_type: "noteOn".into(),
            field: "velocity".into(),
        }
    );
}

#[test]
fn it_should_reject_unexpected_fields() {
    let schema = midi_schema();
    let p = payload(&[
        ("type", FieldValue::String("noteOn".into())),
        ("channel", FieldValue::UInt(1)),
        ("note", FieldValue::UInt(60)),
        ("velocity", FieldValue::UInt(100)),
        ("ghost", FieldValue::UInt(0)),
    ]);

    let err = check_event("midi", &schema, p).expect_err("unexpected field");
    assert_eq!(
        err,
        RuntimeCheckError::UnexpectedField {
            event_type: "noteOn".into(),
            field: "ghost".into(),
        }
    );
}

#[test]
fn it_should_reject_values_outside_the_declared_range() {
    let schema = midi_schema();
    let p = payload(&[
        ("type", FieldValue::String("noteOn".into())),
        ("channel", FieldValue::UInt(0)), // range [1, 16] の外
        ("note", FieldValue::UInt(60)),
        ("velocity", FieldValue::UInt(100)),
    ]);

    let err = check_event("midi", &schema, p).expect_err("range violation");
    let RuntimeCheckError::OutOfRange {
        event_type, field, ..
    } = err
    else {
        panic!("expected OutOfRange, got {err:?}");
    };
    assert_eq!(event_type, "noteOn");
    assert_eq!(field, "channel");
}

#[test]
fn it_should_reject_values_outside_the_types_default_range_when_no_explicit_range() {
    // `range:` 宣言が無いケースでも型のデフォルト値域に収まらない値は弾く必要がある。
    let yaml = r"
schema_version: 1
events:
  raw:
    fields:
      v: { type: uint8 }
    binding_filter: [type]
";
    let schema: EventsSchema = serde_yml::from_str(yaml).expect("parse");
    let p = payload(&[
        ("type", FieldValue::String("raw".into())),
        ("v", FieldValue::UInt(300)), // uint8 default は 0..=255
    ]);

    let err = check_event("drv", &schema, p).expect_err("uint8 default range");
    assert!(matches!(err, RuntimeCheckError::OutOfRange { .. }));
}

#[test]
fn it_should_reject_signed_negative_for_unsigned_field() {
    let schema = midi_schema();
    let p = payload(&[
        ("type", FieldValue::String("noteOn".into())),
        ("channel", FieldValue::Int(-1)),
        ("note", FieldValue::UInt(60)),
        ("velocity", FieldValue::UInt(100)),
    ]);

    let err = check_event("midi", &schema, p).expect_err("negative for uint");
    assert!(
        matches!(err, RuntimeCheckError::TypeMismatch { .. }),
        "expected TypeMismatch, got {err:?}"
    );
}

#[test]
fn it_should_reject_enum_values_not_in_the_declared_list() {
    let schema = midi_schema();
    let p = payload(&[
        ("type", FieldValue::String("realtime".into())),
        ("message", FieldValue::String("rewind".into())),
    ]);

    let err = check_event("midi", &schema, p).expect_err("bad enum value");
    let RuntimeCheckError::EnumValueNotAllowed { value, .. } = err else {
        panic!("expected EnumValueNotAllowed, got {err:?}");
    };
    assert_eq!(value, "rewind");
}

#[test]
fn it_should_reject_bytes_payloads_that_exceed_max_length() {
    let schema = midi_schema();
    let p = payload(&[
        ("type", FieldValue::String("sysex".into())),
        ("payload", FieldValue::Bytes(vec![0u8; 9])), // max_length: 8
    ]);

    let err = check_event("midi", &schema, p).expect_err("max_length");
    assert_eq!(
        err,
        RuntimeCheckError::MaxLengthExceeded {
            event_type: "sysex".into(),
            field: "payload".into(),
            len: 9,
            max: 8,
        }
    );
}

#[test]
fn it_should_reject_field_with_wrong_kind() {
    let schema = midi_schema();
    let p = payload(&[
        ("type", FieldValue::String("realtime".into())),
        ("message", FieldValue::UInt(0)), // enum 期待で UInt
    ]);

    let err = check_event("midi", &schema, p).expect_err("type mismatch");
    let RuntimeCheckError::TypeMismatch {
        expected, actual, ..
    } = err
    else {
        panic!("expected TypeMismatch, got {err:?}");
    };
    assert_eq!(expected, "enum");
    assert_eq!(actual, "uint");
}

#[test]
fn it_should_reject_uint_value_above_i64_max_for_int64_field() {
    // f64 経由の比較だと `i64::MAX` と `i64::MAX as u64 + 1` が同じ値に丸まり、
    // out-of-range が誤って通過する。整数比較で確実に reject されることを担保する。
    let yaml = r"
schema_version: 1
events:
  ping:
    fields:
      v: { type: int64 }
    binding_filter: [type]
";
    let schema: EventsSchema = serde_yml::from_str(yaml).expect("parse");

    #[allow(clippy::cast_sign_loss)]
    let just_above_i64_max: u64 = i64::MAX as u64 + 1;
    let p = payload(&[
        ("type", FieldValue::String("ping".into())),
        ("v", FieldValue::UInt(just_above_i64_max)),
    ]);

    let err = check_event("drv", &schema, p).expect_err("UInt > i64::MAX must be rejected");
    assert!(
        matches!(err, RuntimeCheckError::OutOfRange { .. }),
        "expected OutOfRange, got {err:?}"
    );
}

#[test]
fn it_should_accept_i64_max_as_int64_field() {
    let yaml = r"
schema_version: 1
events:
  ping:
    fields:
      v: { type: int64 }
    binding_filter: [type]
";
    let schema: EventsSchema = serde_yml::from_str(yaml).expect("parse");

    #[allow(clippy::cast_sign_loss)]
    let i64_max_as_u: u64 = i64::MAX as u64;
    let p = payload(&[
        ("type", FieldValue::String("ping".into())),
        ("v", FieldValue::UInt(i64_max_as_u)),
    ]);

    check_event("drv", &schema, p).expect("i64::MAX exact must pass");
}

#[test]
fn it_should_reject_uint64_value_above_explicit_range_with_integer_precision() {
    // `range: [0, 9000000000000000000]` の境界 +1 は f64 では同一に丸まるが、
    // 整数比較なら確実に弾ける。
    let yaml = r"
schema_version: 1
events:
  ping:
    fields:
      v: { type: uint64, range: [0, 9000000000000000000] }
    binding_filter: [type]
";
    let schema: EventsSchema = serde_yml::from_str(yaml).expect("parse");

    let p = payload(&[
        ("type", FieldValue::String("ping".into())),
        ("v", FieldValue::UInt(9_000_000_000_000_000_001)),
    ]);

    let err = check_event("drv", &schema, p).expect_err("just-above range must be rejected");
    assert!(matches!(err, RuntimeCheckError::OutOfRange { .. }));
}

#[test]
fn it_should_reject_negative_signed_for_uint64_field() {
    let yaml = r"
schema_version: 1
events:
  ping:
    fields:
      v: { type: uint64 }
    binding_filter: [type]
";
    let schema: EventsSchema = serde_yml::from_str(yaml).expect("parse");

    let p = payload(&[
        ("type", FieldValue::String("ping".into())),
        ("v", FieldValue::Int(-1)),
    ]);

    let err = check_event("drv", &schema, p).expect_err("negative for uint64");
    assert!(matches!(err, RuntimeCheckError::TypeMismatch { .. }));
}

#[test]
fn it_should_apply_defaults_block_required_check_to_every_event() {
    // defaults 宣言の required field は全 event で required 扱い。
    let yaml = r"
schema_version: 1
defaults:
  ts: { type: uint64 }
events:
  ping:
    fields:
      payload: { type: uint8 }
    binding_filter: [type]
";
    let schema: EventsSchema = serde_yml::from_str(yaml).expect("parse");

    // ts 抜けは required missing
    let missing = payload(&[
        ("type", FieldValue::String("ping".into())),
        ("payload", FieldValue::UInt(7)),
    ]);
    let err = check_event("drv", &schema, missing).expect_err("ts required");
    assert_eq!(
        err,
        RuntimeCheckError::RequiredFieldMissing {
            event_type: "ping".into(),
            field: "ts".into(),
        }
    );

    // ts 同梱なら通る
    let ok = payload(&[
        ("type", FieldValue::String("ping".into())),
        ("payload", FieldValue::UInt(7)),
        ("ts", FieldValue::UInt(123_456)),
    ]);
    check_event("drv", &schema, ok).expect("with ts passes");
}

// ============================================================
// pipeline glue tests (integration)
// ============================================================

#[test]
fn it_should_route_a_valid_event_all_the_way_to_the_layer2_stub() {
    let schema = midi_schema();
    let bytes = encode_map(&[
        ("type", Value::String("noteOn".into())),
        ("channel", Value::from(1u8)),
        ("note", Value::from(60u8)),
        ("velocity", Value::from(100u8)),
    ]);

    let mut sink = RecordingSink::default();
    process_inline_payload("midi", Some(&schema), &bytes, &mut sink);

    assert_eq!(sink.events.len(), 1);
    assert_eq!(sink.events[0].event_type, "noteOn");
    assert_eq!(sink.events[0].driver_name, "midi");
}

#[test]
fn it_should_drop_malformed_msgpack_without_invoking_the_sink() {
    let schema = midi_schema();
    let bogus = vec![0xc1u8]; // msgpack reserved tag → parse error

    let mut sink = RecordingSink::default();
    process_inline_payload("midi", Some(&schema), &bogus, &mut sink);

    assert!(sink.events.is_empty(), "decode failure must drop the event");

    let err = try_process("midi", Some(&schema), &bogus).expect_err("decode fails");
    assert!(matches!(err, ProcessError::Decode(_)));
}

#[test]
fn it_should_drop_schema_violating_events_without_invoking_the_sink() {
    let schema = midi_schema();
    let bytes = encode_map(&[
        ("type", Value::String("noteOn".into())),
        ("channel", Value::from(1u8)),
        ("note", Value::from(60u8)),
        ("velocity", Value::from(200u8)), // range 外
    ]);

    let mut sink = RecordingSink::default();
    process_inline_payload("midi", Some(&schema), &bytes, &mut sink);

    assert!(sink.events.is_empty());

    let err = try_process("midi", Some(&schema), &bytes).expect_err("schema fails");
    let ProcessError::Check(check_err) = err else {
        panic!("expected ProcessError::Check, got {err:?}");
    };
    assert!(matches!(check_err, RuntimeCheckError::OutOfRange { .. }));
}

#[test]
fn it_should_continue_processing_after_dropping_an_invalid_event() {
    // 不正イベントを drop した後でも次の有効イベントが sink に届く（=
    // pipeline は止まらない）ことを連続呼び出しで担保する。同じ sink を
    // 共有することで「process_inline_payload が内部状態を持たず、失敗が
    // 後続呼び出しを汚染しない」契約を回帰として固定する。
    let schema = midi_schema();
    let mut sink = RecordingSink::default();

    // 1 件目: schema 違反（velocity が range 外）
    let invalid = encode_map(&[
        ("type", Value::String("noteOn".into())),
        ("channel", Value::from(1u8)),
        ("note", Value::from(60u8)),
        ("velocity", Value::from(200u8)),
    ]);
    process_inline_payload("midi", Some(&schema), &invalid, &mut sink);
    assert!(
        sink.events.is_empty(),
        "schema 違反は sink に届かない（drop）"
    );

    // 2 件目: 正常イベント
    let valid = encode_map(&[
        ("type", Value::String("noteOn".into())),
        ("channel", Value::from(1u8)),
        ("note", Value::from(60u8)),
        ("velocity", Value::from(100u8)),
    ]);
    process_inline_payload("midi", Some(&schema), &valid, &mut sink);

    assert_eq!(
        sink.events.len(),
        1,
        "drop 後も後続イベントは dispatch される"
    );
    assert_eq!(sink.events[0].event_type, "noteOn");
}

#[test]
fn it_should_render_decode_error_display_with_specific_diagnostic_text() {
    let bytes = encode_map(&[
        ("type", Value::String("noteOn".into())),
        (
            "inner",
            Value::Map(vec![(Value::String("x".into()), Value::from(1u8))]),
        ),
    ]);
    let err = decode_event(&bytes).expect_err("nested map");
    let rendered = err.to_string();
    assert!(
        rendered.contains("nested maps"),
        "DecodeError display should describe the violation, got: {rendered}"
    );
}

#[test]
fn it_should_render_runtime_check_error_display_with_field_and_event_context() {
    let schema = midi_schema();
    let p = payload(&[
        ("type", FieldValue::String("noteOn".into())),
        ("channel", FieldValue::UInt(99)), // out of [1, 16]
        ("note", FieldValue::UInt(60)),
        ("velocity", FieldValue::UInt(100)),
    ]);
    let err = check_event("midi", &schema, p).expect_err("range");
    let rendered = err.to_string();
    assert!(
        rendered.contains("noteOn"),
        "RuntimeCheckError display should mention the event type, got: {rendered}"
    );
    assert!(
        rendered.contains("channel"),
        "RuntimeCheckError display should mention the offending field, got: {rendered}"
    );
}

#[test]
fn it_should_render_process_error_display_distinguishing_each_pipeline_stage() {
    let schema = midi_schema();
    let bogus_msgpack = vec![0xc1u8];
    let decode_err = try_process("midi", Some(&schema), &bogus_msgpack).expect_err("decode");
    assert!(
        decode_err.to_string().contains("msgpack decode failed"),
        "ProcessError::Decode display, got: {decode_err}"
    );

    let missing_err = try_process("midi", None, &[0x80u8]).expect_err("missing");
    assert!(
        missing_err
            .to_string()
            .contains("driver has no events.yaml schema loaded"),
        "ProcessError::SchemaMissing display, got: {missing_err}"
    );

    let bytes = encode_map(&[
        ("type", Value::String("noteOn".into())),
        ("channel", Value::from(1u8)),
        ("note", Value::from(60u8)),
        ("velocity", Value::from(200u8)), // out of range
    ]);
    let check_err = try_process("midi", Some(&schema), &bytes).expect_err("check");
    assert!(
        check_err.to_string().contains("schema check failed"),
        "ProcessError::Check display, got: {check_err}"
    );
}

#[test]
fn it_should_drop_events_when_the_drivers_schema_is_unavailable() {
    let bytes = encode_map(&[
        ("type", Value::String("noteOn".into())),
        ("channel", Value::from(1u8)),
        ("note", Value::from(60u8)),
        ("velocity", Value::from(100u8)),
    ]);

    let mut sink = RecordingSink::default();
    process_inline_payload("midi", None, &bytes, &mut sink);

    assert!(
        sink.events.is_empty(),
        "events.yaml が無い driver の event は全件 drop される"
    );

    let err = try_process("midi", None, &bytes).expect_err("schema missing");
    assert!(matches!(err, ProcessError::SchemaMissing));
}
