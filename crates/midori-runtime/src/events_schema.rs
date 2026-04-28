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

// この module の公開 API は Bridge パイプラインからまだ呼び出されておらず
// （後続 subtask で接続予定）、binary crate 内の dead_code 検出に引っかかる
// ため module 全体で抑制する。実体は単体テストで網羅している。
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

// =============================================================================
// Schema types
// =============================================================================

/// Top-level structure of `events.yaml`.
#[derive(Debug, Deserialize, PartialEq)]
pub struct EventsSchema {
    pub schema_version: u32,
    pub events: BTreeMap<String, EventDef>,
    #[serde(default)]
    pub defaults: BTreeMap<String, FieldSpec>,
}

/// Per-event definition.
#[derive(Debug, Deserialize, PartialEq)]
pub struct EventDef {
    pub fields: BTreeMap<String, FieldSpec>,
    #[serde(default)]
    pub tier: Tier,
    #[serde(default)]
    pub binding_filter: Vec<String>,
    #[serde(default)]
    pub note_field: Option<String>,
}

/// Delivery tier for an event.
///
/// `inline` (default) flows through the shm SPSC ring; `streamed` flows
/// through a future non-shm channel. The runtime feature-availability check
/// for `streamed` is implemented separately.
#[derive(Debug, Default, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    #[default]
    Inline,
    Streamed,
}

/// Field declaration (`field_spec` in spec terms).
#[derive(Debug, Deserialize, PartialEq)]
pub struct FieldSpec {
    #[serde(rename = "type")]
    pub ty: FieldType,
    #[serde(default)]
    pub range: Option<RangeBound>,
    #[serde(default)]
    pub values: Option<Vec<String>>,
    #[serde(default)]
    pub max_length: Option<u64>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub default: Option<serde_yml::Value>,
}

/// `range: [min, max]` bound. Stored as raw YAML values; validation checks
/// numeric compatibility with the declared field type.
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(try_from = "Vec<serde_yml::Value>")]
pub struct RangeBound {
    pub min: serde_yml::Value,
    pub max: serde_yml::Value,
}

impl TryFrom<Vec<serde_yml::Value>> for RangeBound {
    type Error = String;
    fn try_from(v: Vec<serde_yml::Value>) -> Result<Self, Self::Error> {
        let mut iter = v.into_iter();
        let min = iter
            .next()
            .ok_or_else(|| "range must have exactly 2 elements".to_owned())?;
        let max = iter
            .next()
            .ok_or_else(|| "range must have exactly 2 elements".to_owned())?;
        if iter.next().is_some() {
            return Err("range must have exactly 2 elements".to_owned());
        }
        Ok(Self { min, max })
    }
}

/// events.yaml field type vocabulary.
///
/// MIDI / OSC のドメイン固有値域は generic 整数型 + `range` で表現する規約
/// （`design/16-driver-events-schema.md`「driver ドメイン慣例の表現」節）。
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FieldType {
    Int8,
    Uint8,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Int64,
    Uint64,
    Float32,
    Float64,
    Bool,
    String,
    Bytes,
    Enum,
    Array(Box<FieldType>),
}

impl<'de> Deserialize<'de> for FieldType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown type vocabulary: {s}")))
    }
}

impl FieldType {
    fn parse(s: &str) -> Option<Self> {
        if let Some(inner) = s.strip_prefix("array<").and_then(|x| x.strip_suffix('>')) {
            return Self::parse(inner.trim()).map(|t| Self::Array(Box::new(t)));
        }
        Some(match s {
            "int8" => Self::Int8,
            "uint8" => Self::Uint8,
            "int16" => Self::Int16,
            "uint16" => Self::Uint16,
            "int32" => Self::Int32,
            "uint32" => Self::Uint32,
            "int64" => Self::Int64,
            "uint64" => Self::Uint64,
            "float32" => Self::Float32,
            "float64" => Self::Float64,
            "bool" => Self::Bool,
            "string" => Self::String,
            "bytes" => Self::Bytes,
            "enum" => Self::Enum,
            _ => return None,
        })
    }

    /// Whether the type accepts a numeric `range` constraint.
    fn supports_range(&self) -> bool {
        matches!(
            self,
            Self::Int8
                | Self::Uint8
                | Self::Int16
                | Self::Uint16
                | Self::Int32
                | Self::Uint32
                | Self::Int64
                | Self::Uint64
                | Self::Float32
                | Self::Float64
        )
    }

    /// Whether the type accepts `max_length` (variable-length payloads).
    fn supports_max_length(&self) -> bool {
        matches!(self, Self::String | Self::Bytes | Self::Array(_))
    }

    /// Whether the type can be referenced by `binding_filter`.
    /// Variable-length / structured types are excluded.
    fn is_filterable(&self) -> bool {
        !matches!(self, Self::Bytes | Self::Array(_))
    }

    /// Default integer/float bounds (inclusive). `None` for non-numeric types.
    /// 整数の `f64` 化は schema validator の境界比較用なので、`i64::MAX`
    /// 付近で 1 ulp ずれても実害がない（events.yaml で `int64` ぴったり
    /// 境界の `range` を書く driver は事実上いない）と判断して許容する。
    #[allow(clippy::cast_precision_loss)]
    fn default_range(&self) -> Option<(f64, f64)> {
        Some(match self {
            Self::Int8 => (f64::from(i8::MIN), f64::from(i8::MAX)),
            Self::Uint8 => (0.0, f64::from(u8::MAX)),
            Self::Int16 => (f64::from(i16::MIN), f64::from(i16::MAX)),
            Self::Uint16 => (0.0, f64::from(u16::MAX)),
            Self::Int32 => (f64::from(i32::MIN), f64::from(i32::MAX)),
            Self::Uint32 => (0.0, f64::from(u32::MAX)),
            Self::Int64 => (i64::MIN as f64, i64::MAX as f64),
            Self::Uint64 => (0.0, u64::MAX as f64),
            Self::Float32 => (f64::from(f32::MIN), f64::from(f32::MAX)),
            Self::Float64 => (f64::MIN, f64::MAX),
            _ => return None,
        })
    }
}

// =============================================================================
// Loader
// =============================================================================

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

impl std::error::Error for LoadError {}

/// Load `events.yaml` from `path`. Returns `LoadOutcome::Missing` when the
/// file does not exist (per spec the caller decides drop-all vs warning).
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

// =============================================================================
// Validator
// =============================================================================

/// Single schema violation. Path identifies the offending location.
#[derive(Debug, PartialEq, Eq)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Validate `schema` against the rules in `design/16-driver-events-schema.md`.
/// Returns all violations collected (does not stop at the first error).
pub fn validate(schema: &EventsSchema) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    if schema.events.is_empty() {
        errors.push(ValidationError {
            path: "events".to_owned(),
            message: "must have at least one event definition".to_owned(),
        });
    }

    for (field_name, spec) in &schema.defaults {
        validate_field_spec(&format!("defaults.{field_name}"), spec, &mut errors);
    }

    for (event_name, event) in &schema.events {
        validate_event(event_name, event, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_event(name: &str, event: &EventDef, errors: &mut Vec<ValidationError>) {
    let event_path = format!("events.{name}");

    if event.fields.is_empty() {
        errors.push(ValidationError {
            path: format!("{event_path}.fields"),
            message: "must have at least one field".to_owned(),
        });
    }

    let field_names: BTreeSet<&str> = event.fields.keys().map(String::as_str).collect();

    // Per-field validation
    for (field_name, spec) in &event.fields {
        validate_field_spec(&format!("{event_path}.fields.{field_name}"), spec, errors);
    }

    // note_field reference must exist in fields
    if let Some(note_field) = &event.note_field {
        if !field_names.contains(note_field.as_str()) {
            errors.push(ValidationError {
                path: format!("{event_path}.note_field"),
                message: format!("references unknown field `{note_field}`"),
            });
        }
    }

    // binding_filter references must exist in fields (or be `type`, the
    // implicit event-type discriminator)
    for filter in &event.binding_filter {
        if filter == "type" {
            continue;
        }
        let Some(spec) = event.fields.get(filter) else {
            errors.push(ValidationError {
                path: format!("{event_path}.binding_filter"),
                message: format!("references unknown field `{filter}`"),
            });
            continue;
        };
        if !spec.ty.is_filterable() {
            errors.push(ValidationError {
                path: format!("{event_path}.binding_filter"),
                message: format!(
                    "field `{filter}` has non-filterable type (variable-length or structured)"
                ),
            });
        }
    }
}

fn validate_field_spec(path: &str, spec: &FieldSpec, errors: &mut Vec<ValidationError>) {
    // enum requires `values`
    if matches!(spec.ty, FieldType::Enum) && spec.values.is_none() {
        errors.push(ValidationError {
            path: path.to_owned(),
            message: "type `enum` requires `values` list".to_owned(),
        });
    }

    // `values` only for enum
    if !matches!(spec.ty, FieldType::Enum) && spec.values.is_some() {
        errors.push(ValidationError {
            path: path.to_owned(),
            message: "`values` is only valid with type `enum`".to_owned(),
        });
    }

    // optional: false and default conflict
    if !spec.optional && spec.default.is_some() {
        errors.push(ValidationError {
            path: path.to_owned(),
            message: "`default` requires `optional: true`".to_owned(),
        });
    }

    // max_length applicable type + value > 0
    if let Some(len) = spec.max_length {
        if !spec.ty.supports_max_length() {
            errors.push(ValidationError {
                path: path.to_owned(),
                message: "`max_length` is only valid with `string` / `bytes` / `array<T>`"
                    .to_owned(),
            });
        }
        if len == 0 {
            errors.push(ValidationError {
                path: path.to_owned(),
                message: "`max_length` must be positive".to_owned(),
            });
        }
    }

    // range validation
    if let Some(range) = &spec.range {
        validate_range(path, &spec.ty, range, errors);
    }
}

fn validate_range(
    path: &str,
    ty: &FieldType,
    range: &RangeBound,
    errors: &mut Vec<ValidationError>,
) {
    if !ty.supports_range() {
        errors.push(ValidationError {
            path: path.to_owned(),
            message: "`range` is only valid with numeric types".to_owned(),
        });
        return;
    }
    let (Some(min), Some(max)) = (yaml_to_f64(&range.min), yaml_to_f64(&range.max)) else {
        errors.push(ValidationError {
            path: path.to_owned(),
            message: "`range` values must be numeric".to_owned(),
        });
        return;
    };
    if min > max {
        errors.push(ValidationError {
            path: path.to_owned(),
            message: format!("`range` min ({min}) must be <= max ({max})"),
        });
    }
    if let Some((dmin, dmax)) = ty.default_range() {
        if min < dmin || max > dmax {
            errors.push(ValidationError {
                path: path.to_owned(),
                message: format!(
                    "`range` [{min}, {max}] is outside the default value range [{dmin}, {dmax}] for the declared type"
                ),
            });
        }
    }
}

fn yaml_to_f64(v: &serde_yml::Value) -> Option<f64> {
    match v {
        serde_yml::Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
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
        let path = std::env::temp_dir().join("nonexistent-events.yaml");
        let _ = std::fs::remove_file(&path);
        let outcome = load_from_path(&path).expect("missing file should not error");
        assert!(matches!(outcome, LoadOutcome::Missing));
    }

    #[test]
    fn it_should_load_existing_file_as_outcome_loaded() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("midori-events-test-{}.yaml", std::process::id()));
        std::fs::write(
            &path,
            r"
schema_version: 1
events:
  evt:
    fields:
      x: { type: uint8 }
",
        )
        .expect("write tmp");
        let outcome = load_from_path(&path).expect("load");
        let _ = std::fs::remove_file(&path);
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
}
