//! events.yaml schema sub-module 全体の単体テスト。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;

fn parse(yaml: &str) -> EventsSchema {
    serde_yml::from_str(yaml).expect("parse should succeed")
}

#[test]
fn it_should_parse_minimal_valid_schema() {
    let schema = parse(
        r"
schema_version: 1
events:
  noteOn:
    fields:
      channel: { type: uint8, range: [1, 16] }
      note: { type: uint8, range: [0, 127] }
      velocity: { type: uint8, range: [0, 127] }
    binding_filter: [type, channel]
    note_field: note
",
    );
    assert_eq!(schema.schema_version, 1);
    assert!(schema.events.contains_key("noteOn"));
    assert_eq!(schema.events["noteOn"].tier, Tier::Inline);
}

#[test]
fn it_should_default_tier_to_inline_when_omitted() {
    let schema = parse(
        r"
schema_version: 1
events:
  noteOn:
    fields:
      x: { type: uint8 }
",
    );
    assert_eq!(schema.events["noteOn"].tier, Tier::Inline);
}

#[test]
fn it_should_parse_streamed_tier() {
    let schema = parse(
        r"
schema_version: 1
events:
  oscBlob:
    tier: streamed
    fields:
      payload: { type: bytes, max_length: 65536 }
",
    );
    assert_eq!(schema.events["oscBlob"].tier, Tier::Streamed);
}

#[test]
fn it_should_reject_unknown_tier_value() {
    let yaml = r"
schema_version: 1
events:
  evt:
    tier: turbo
    fields:
      x: { type: uint8 }
";
    let result: Result<EventsSchema, _> = serde_yml::from_str(yaml);
    assert!(result.is_err(), "unknown tier should fail to deserialize");
}

#[test]
fn it_should_reject_non_string_tier() {
    let yaml = r"
schema_version: 1
events:
  evt:
    tier: true
    fields:
      x: { type: uint8 }
";
    let result: Result<EventsSchema, _> = serde_yml::from_str(yaml);
    assert!(
        result.is_err(),
        "non-string tier should fail to deserialize"
    );
}

#[test]
fn it_should_reject_unknown_field_type_at_parse_time() {
    let yaml = r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: weirdType }
";
    let result: Result<EventsSchema, _> = serde_yml::from_str(yaml);
    assert!(result.is_err(), "unknown type vocabulary should fail");
}

#[test]
fn it_should_parse_array_type() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      vals: { type: array<uint8>, max_length: 16 }
",
    );
    let spec = &schema.events["evt"].fields["vals"];
    assert!(matches!(spec.ty, FieldType::Array(ref t) if **t == FieldType::Uint8));
}

#[test]
fn it_should_validate_minimal_schema() {
    let schema = parse(
        r"
schema_version: 1
events:
  noteOn:
    fields:
      channel: { type: uint8, range: [1, 16] }
      note: { type: uint8, range: [0, 127] }
",
    );
    validate(&schema).expect("minimal schema should validate");
}

#[test]
fn it_should_reject_empty_events_map() {
    let schema = EventsSchema {
        schema_version: 1,
        events: BTreeMap::new(),
        defaults: BTreeMap::new(),
    };
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.path == "events"));
}

#[test]
fn it_should_reject_event_with_no_fields() {
    let schema = parse(
        r"
schema_version: 1
events:
  empty:
    fields: {}
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.path == "events.empty.fields"));
}

#[test]
fn it_should_reject_range_min_greater_than_max() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8, range: [10, 5] }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("min")));
}

#[test]
fn it_should_reject_range_outside_default_bounds() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8, range: [-1, 200] }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("outside")));
}

#[test]
fn it_should_reject_range_on_non_numeric_type() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      flag: { type: bool, range: [0, 1] }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("numeric")));
}

#[test]
fn it_should_reject_max_length_on_non_variable_type() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8, max_length: 16 }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("max_length")));
}

#[test]
fn it_should_reject_zero_max_length() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      payload: { type: bytes, max_length: 0 }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("positive")));
}

#[test]
fn it_should_reject_duplicate_event_names_at_parse_time() {
    let yaml = r"
schema_version: 1
events:
  noteOn:
    fields:
      x: { type: uint8 }
  noteOn:
    fields:
      y: { type: uint8 }
";
    let result: Result<EventsSchema, _> = serde_yml::from_str(yaml);
    assert!(
        result.is_err(),
        "duplicate event name should fail to deserialize"
    );
}

#[test]
fn it_should_reject_array_of_enum() {
    let yaml = r"
schema_version: 1
events:
  evt:
    fields:
      vals: { type: array<enum>, max_length: 4 }
";
    let result: Result<EventsSchema, _> = serde_yml::from_str(yaml);
    assert!(
        result.is_err(),
        "array<enum> should fail (non-scalar inner type)"
    );
}

#[test]
fn it_should_reject_nested_array_type() {
    let yaml = r"
schema_version: 1
events:
  evt:
    fields:
      vals: { type: array<array<uint8>>, max_length: 4 }
";
    let result: Result<EventsSchema, _> = serde_yml::from_str(yaml);
    assert!(
        result.is_err(),
        "nested array type should fail (non-scalar inner type)"
    );
}

#[test]
fn it_should_reject_non_numeric_range_values() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8, range: [foo, bar] }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("numeric")),
        "non-numeric range values should be rejected"
    );
}

#[test]
fn it_should_reject_nan_range_values() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: float64, range: [.nan, .nan] }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("finite")),
        "NaN range values should be rejected"
    );
}

#[test]
fn it_should_reject_unknown_top_level_field() {
    let yaml = r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8 }
unknownTopLevel: oops
";
    let result: Result<EventsSchema, _> = serde_yml::from_str(yaml);
    assert!(
        result.is_err(),
        "unknown top-level field should be rejected"
    );
}

#[test]
fn it_should_reject_unknown_field_in_event_def() {
    let yaml = r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8 }
    unknownProp: 1
";
    let result: Result<EventsSchema, _> = serde_yml::from_str(yaml);
    assert!(result.is_err(), "unknown event field should be rejected");
}

#[test]
fn it_should_reject_duplicate_fields_in_event() {
    let yaml = r"
schema_version: 1
events:
  evt:
    fields:
      a: { type: uint8 }
      a: { type: uint8 }
";
    let result: Result<EventsSchema, _> = serde_yml::from_str(yaml);
    assert!(
        result.is_err(),
        "duplicate field in event should fail to deserialize"
    );
}

#[test]
fn it_should_resolve_note_field_via_defaults() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8 }
    note_field: common
defaults:
  common: { type: uint8 }
",
    );
    validate(&schema).expect("note_field referencing defaults should resolve");
}

#[test]
fn it_should_resolve_binding_filter_via_defaults() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8 }
    binding_filter: [type, common]
defaults:
  common: { type: uint8 }
",
    );
    validate(&schema).expect("binding_filter referencing defaults should resolve");
}

#[test]
fn it_should_reject_default_with_mismatched_type() {
    let schema = parse(
        r#"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8, optional: true, default: "abc" }
"#,
    );
    let errors = validate(&schema).unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("type")),
        "default with wrong type should be rejected"
    );
}

#[test]
fn it_should_reject_default_outside_range() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8, range: [0, 127], optional: true, default: 200 }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("outside")),
        "default outside range should be rejected"
    );
}

#[test]
fn it_should_reject_default_not_in_enum_values() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      mode: { type: enum, values: [a, b, c], optional: true, default: zz }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("not in")),
        "default not in enum values should be rejected"
    );
}

#[test]
fn it_should_validate_defaults_field_specs() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8 }
defaults:
  bad: { type: uint8, range: [10, 5] }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(
        errors.iter().any(|e| e.path == "defaults.bad"),
        "defaults.bad should be reported"
    );
}

#[test]
fn it_should_reject_default_with_optional_false() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8, default: 5 }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("default")));
}

#[test]
fn it_should_accept_default_with_optional_true() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8, optional: true, default: 5 }
",
    );
    validate(&schema).expect("optional default should be valid");
}

#[test]
fn it_should_reject_enum_without_values() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      mode: { type: enum }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("values")));
}

#[test]
fn it_should_reject_values_on_non_enum() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8, values: [a, b] }
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("values")));
}

#[test]
fn it_should_reject_unknown_note_field_reference() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8 }
    note_field: missing
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("unknown field")));
}

#[test]
fn it_should_reject_unknown_binding_filter_reference() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8 }
    binding_filter: [type, missing]
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("unknown field")));
}

#[test]
fn it_should_reject_non_filterable_type_in_binding_filter() {
    let schema = parse(
        r"
schema_version: 1
events:
  evt:
    fields:
      payload: { type: bytes, max_length: 64 }
    binding_filter: [type, payload]
",
    );
    let errors = validate(&schema).unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("non-filterable")));
}

#[test]
fn it_should_load_missing_file_as_outcome_missing() {
    // 一意な temp dir を作って、その中の未作成 path を渡す。並列実行や
    // 既存ファイルとの衝突を防ぐ（tempdir は Drop で auto cleanup）。
    let dir = tempfile::tempdir().expect("create tmp dir");
    let path = dir.path().join("missing.yaml");
    let outcome = load_from_path(&path).expect("missing file should not error");
    assert!(matches!(outcome, LoadOutcome::Missing));
}

#[test]
fn it_should_load_existing_file_as_outcome_loaded() {
    // NamedTempFile は Drop 時に自動削除されるため、assert が panic
    // しても tmp ファイルは残らない。
    let file = tempfile::NamedTempFile::new().expect("create tmp");
    std::fs::write(
        file.path(),
        r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8 }
",
    )
    .expect("write tmp");
    let outcome = load_from_path(file.path()).expect("load");
    match outcome {
        LoadOutcome::Loaded(schema) => assert_eq!(schema.schema_version, 1),
        LoadOutcome::Missing => panic!("should be Loaded"),
    }
}

#[test]
fn it_should_resolve_events_yaml_next_to_driver_yaml() {
    let dir = Path::new("/path/to/drivers/midi");
    assert_eq!(
        resolve_events_yaml_path(dir),
        PathBuf::from("/path/to/drivers/midi/events.yaml")
    );
}
