//! events.yaml schema 違反検出ルール。

use std::collections::BTreeMap;

use super::types::{EventDef, EventsSchema, FieldSpec, FieldType, RangeBound};

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

impl std::error::Error for ValidationError {}

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
        validate_event(event_name, event, &schema.defaults, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_event(
    name: &str,
    event: &EventDef,
    defaults: &BTreeMap<String, FieldSpec>,
    errors: &mut Vec<ValidationError>,
) {
    let event_path = format!("events.{name}");

    if event.fields.is_empty() {
        errors.push(ValidationError {
            path: format!("{event_path}.fields"),
            message: "must have at least one field".to_owned(),
        });
    }

    // 参照解決用の merged view: defaults を base に置き、event.fields で上書き。
    // spec「全イベント共通フィールドのデフォルト宣言」に従い、defaults の field
    // 名は note_field / binding_filter から参照可能。
    let merged: BTreeMap<&str, &FieldSpec> = defaults
        .iter()
        .chain(event.fields.iter())
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    // Per-field validation (event.fields のみ。defaults は親 validator が直接検証)
    for (field_name, spec) in &event.fields {
        validate_field_spec(&format!("{event_path}.fields.{field_name}"), spec, errors);
    }

    // note_field reference must exist in fields ∪ defaults
    if let Some(note_field) = &event.note_field {
        if !merged.contains_key(note_field.as_str()) {
            errors.push(ValidationError {
                path: format!("{event_path}.note_field"),
                message: format!("references unknown field `{note_field}`"),
            });
        }
    }

    // binding_filter references must exist in fields ∪ defaults (or be `type`,
    // the implicit event-type discriminator)
    for filter in &event.binding_filter {
        if filter == "type" {
            continue;
        }
        let Some(spec) = merged.get(filter.as_str()).copied() else {
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

    // default value 型整合性
    if let Some(default) = &spec.default {
        validate_default(path, spec, default, errors);
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
    if !min.is_finite() || !max.is_finite() {
        errors.push(ValidationError {
            path: path.to_owned(),
            message: "`range` values must be finite (NaN / Inf are rejected)".to_owned(),
        });
        return;
    }
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

fn validate_default(
    path: &str,
    spec: &FieldSpec,
    default: &serde_yml::Value,
    errors: &mut Vec<ValidationError>,
) {
    use serde_yml::Value;
    let push = |msg: String, errors: &mut Vec<ValidationError>| {
        errors.push(ValidationError {
            path: path.to_owned(),
            message: msg,
        });
    };

    // 型 vs YAML kind の最低限の照合
    let kind_ok = matches!(
        (&spec.ty, default),
        (
            FieldType::Int8
                | FieldType::Uint8
                | FieldType::Int16
                | FieldType::Uint16
                | FieldType::Int32
                | FieldType::Uint32
                | FieldType::Int64
                | FieldType::Uint64
                | FieldType::Float32
                | FieldType::Float64,
            Value::Number(_),
        ) | (FieldType::Bool, Value::Bool(_))
            | (FieldType::String | FieldType::Enum, Value::String(_))
            | (FieldType::Bytes, Value::String(_) | Value::Sequence(_))
            | (FieldType::Array(_), Value::Sequence(_))
    );
    if !kind_ok {
        push(
            "`default` value type does not match the declared field type".to_owned(),
            errors,
        );
        return;
    }

    // enum: default が values に含まれているか
    if matches!(spec.ty, FieldType::Enum) {
        if let (Value::String(s), Some(values)) = (default, &spec.values) {
            if !values.iter().any(|v| v == s) {
                push(format!("`default` value `{s}` is not in `values`"), errors);
            }
        }
    }

    // 数値: range（あれば）or 型のデフォルト値域に収まる
    if spec.ty.supports_range() {
        if let Some(n) = yaml_to_f64(default) {
            if n.is_finite() {
                let bound = spec
                    .range
                    .as_ref()
                    .and_then(|r| {
                        let lo = yaml_to_f64(&r.min)?;
                        let hi = yaml_to_f64(&r.max)?;
                        if lo.is_finite() && hi.is_finite() && lo <= hi {
                            Some((lo, hi))
                        } else {
                            None
                        }
                    })
                    .or_else(|| spec.ty.default_range());
                if let Some((lo, hi)) = bound {
                    if n < lo || n > hi {
                        push(
                            format!(
                                "`default` value {n} is outside the allowed range [{lo}, {hi}]"
                            ),
                            errors,
                        );
                    }
                }
            } else {
                push(
                    "`default` value must be finite (NaN / Inf are rejected)".to_owned(),
                    errors,
                );
            }
        }
    }

    // string / bytes / array: max_length（あれば）に収まる
    if let Some(max_len) = spec.max_length {
        let len = match (&spec.ty, default) {
            (FieldType::String | FieldType::Bytes, Value::String(s)) => Some(s.len() as u64),
            (FieldType::Bytes | FieldType::Array(_), Value::Sequence(seq)) => {
                Some(seq.len() as u64)
            }
            _ => None,
        };
        if let Some(actual) = len {
            if actual > max_len {
                push(
                    format!("`default` length {actual} exceeds `max_length` {max_len}"),
                    errors,
                );
            }
        }
    }
}

/// `serde_yml::Value` の数値を f64 に変換する。
///
/// `as_f64()` 単独では実装によって整数リテラル (`range: [0, 127]` 等) が
/// `None` 返却となる可能性があるため、`as_i64()` / `as_u64()` への fallback
/// を順に試す。`Value::Number` 以外は `None`。
#[allow(clippy::cast_precision_loss)]
fn yaml_to_f64(v: &serde_yml::Value) -> Option<f64> {
    match v {
        serde_yml::Value::Number(n) => n
            .as_f64()
            .or_else(|| n.as_i64().map(|i| i as f64))
            .or_else(|| n.as_u64().map(|u| u as f64)),
        _ => None,
    }
}
