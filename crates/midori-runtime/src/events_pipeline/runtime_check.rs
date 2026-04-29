//! Decode 後の payload を events.yaml schema と runtime 照合する層。
//!
//! 起動時 schema validator（`events_schema::validator`）が **schema 自身の
//! 整合性** を検証するのに対し、本層は **driver が emit したイベント単位** で
//! schema と raw event を突き合わせる。違反は `RuntimeCheckError` として
//! 上位パイプラインに返し、上位は Error ログを出して drop する。
//!
//! 本層は `optional: true` のフィールドが欠落しているケースに **default 値を
//! 埋める処理** は行わない。default 注入は Layer 2 binding 側で行う方針
//! （wire 上で driver が省略した事実をパイプライン入口で残しておくため）。

use std::collections::BTreeMap;

use super::decode::{DecodedPayload, FieldValue};
use crate::events_schema::{yaml_to_f64, EventDef, EventsSchema, FieldSpec, FieldType, RangeBound};

/// schema 照合を通過したイベント。Layer 2 binding の入口に渡す形。
#[derive(Debug, PartialEq)]
pub struct ValidatedEvent {
    /// emit した driver の論理名（ログ／メトリクス用）。
    pub driver_name: String,
    /// events.yaml の最上位キーと一致するイベント種別。
    pub event_type: String,
    /// 検証済みフィールド。`type` discriminator は外して保持する。
    pub fields: BTreeMap<String, FieldValue>,
}

/// Runtime 照合の失敗種別。
#[derive(Debug, PartialEq)]
pub enum RuntimeCheckError {
    /// payload に `type` フィールドが無い。
    MissingTypeField,
    /// `type` の値が文字列ではない。
    NonStringType,
    /// `type` の値が events.yaml の最上位キーに存在しない。
    UnknownEventType { event_type: String },
    /// `optional: false` のフィールドが payload に無い。
    RequiredFieldMissing { event_type: String, field: String },
    /// schema に存在しないフィールドを driver が emit した。
    UnexpectedField { event_type: String, field: String },
    /// 値の msgpack 表現が宣言された events.yaml 型と互換でない。
    TypeMismatch {
        event_type: String,
        field: String,
        expected: &'static str,
        actual: &'static str,
    },
    /// 数値が `range:` または型のデフォルト値域を外れた。
    OutOfRange {
        event_type: String,
        field: String,
        value: f64,
        lo: f64,
        hi: f64,
    },
    /// `enum` 型の値が `values:` リストに含まれていない。
    EnumValueNotAllowed {
        event_type: String,
        field: String,
        value: String,
    },
    /// `string` / `bytes` / `array<T>` の長さが `max_length` を超えた。
    MaxLengthExceeded {
        event_type: String,
        field: String,
        len: u64,
        max: u64,
    },
}

impl std::fmt::Display for RuntimeCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingTypeField => f.write_str("payload is missing the `type` field"),
            Self::NonStringType => f.write_str("`type` field must be a string"),
            Self::UnknownEventType { event_type } => {
                write!(f, "unknown event type `{event_type}`")
            }
            Self::RequiredFieldMissing { event_type, field } => {
                write!(f, "event `{event_type}` is missing required field `{field}`")
            }
            Self::UnexpectedField { event_type, field } => {
                write!(
                    f,
                    "event `{event_type}` carries unexpected field `{field}`"
                )
            }
            Self::TypeMismatch {
                event_type,
                field,
                expected,
                actual,
            } => write!(
                f,
                "event `{event_type}` field `{field}`: expected `{expected}`, got `{actual}`"
            ),
            Self::OutOfRange {
                event_type,
                field,
                value,
                lo,
                hi,
            } => write!(
                f,
                "event `{event_type}` field `{field}`: value {value} is outside [{lo}, {hi}]"
            ),
            Self::EnumValueNotAllowed {
                event_type,
                field,
                value,
            } => write!(
                f,
                "event `{event_type}` field `{field}`: enum value `{value}` is not in the declared `values:` list"
            ),
            Self::MaxLengthExceeded {
                event_type,
                field,
                len,
                max,
            } => write!(
                f,
                "event `{event_type}` field `{field}`: length {len} exceeds `max_length` {max}"
            ),
        }
    }
}

impl std::error::Error for RuntimeCheckError {}

/// `decoded` を `schema` と照合し、合致すれば [`ValidatedEvent`] を返す。
///
/// `defaults` 宣言のフィールドは「全イベント共通の暗黙フィールド」として
/// 各 event のフィールドにマージして検査する（`event.fields` 側が同名キーで
/// shadow できる、validator と同じ規則）。
pub fn check_event(
    driver_name: &str,
    schema: &EventsSchema,
    decoded: DecodedPayload,
) -> Result<ValidatedEvent, RuntimeCheckError> {
    let event_type = extract_event_type(&decoded)?;
    let event_def =
        schema
            .events
            .get(&event_type)
            .ok_or_else(|| RuntimeCheckError::UnknownEventType {
                event_type: event_type.clone(),
            })?;

    let merged_specs = merge_field_specs(event_def, &schema.defaults);

    check_unexpected_fields(&event_type, &decoded, &merged_specs)?;
    check_required_fields(&event_type, &decoded, &merged_specs)?;
    check_field_values(&event_type, &decoded, &merged_specs)?;

    let mut fields = decoded;
    fields.remove("type");
    Ok(ValidatedEvent {
        driver_name: driver_name.to_owned(),
        event_type,
        fields,
    })
}

fn extract_event_type(decoded: &DecodedPayload) -> Result<String, RuntimeCheckError> {
    let raw = decoded
        .get("type")
        .ok_or(RuntimeCheckError::MissingTypeField)?;
    match raw {
        FieldValue::String(s) => Ok(s.clone()),
        _ => Err(RuntimeCheckError::NonStringType),
    }
}

fn merge_field_specs<'a>(
    event_def: &'a EventDef,
    defaults: &'a BTreeMap<String, FieldSpec>,
) -> BTreeMap<&'a str, &'a FieldSpec> {
    // defaults を base に置き、event.fields で上書き（events_schema::validator と同じ規則）
    defaults
        .iter()
        .chain(event_def.fields.iter())
        .map(|(k, v)| (k.as_str(), v))
        .collect()
}

fn check_unexpected_fields(
    event_type: &str,
    decoded: &DecodedPayload,
    merged: &BTreeMap<&str, &FieldSpec>,
) -> Result<(), RuntimeCheckError> {
    for field_name in decoded.keys() {
        if field_name == "type" {
            continue;
        }
        if !merged.contains_key(field_name.as_str()) {
            return Err(RuntimeCheckError::UnexpectedField {
                event_type: event_type.to_owned(),
                field: field_name.clone(),
            });
        }
    }
    Ok(())
}

fn check_required_fields(
    event_type: &str,
    decoded: &DecodedPayload,
    merged: &BTreeMap<&str, &FieldSpec>,
) -> Result<(), RuntimeCheckError> {
    for (field_name, spec) in merged {
        if spec.optional {
            continue;
        }
        if !decoded.contains_key(*field_name) {
            return Err(RuntimeCheckError::RequiredFieldMissing {
                event_type: event_type.to_owned(),
                field: (*field_name).to_owned(),
            });
        }
    }
    Ok(())
}

fn check_field_values(
    event_type: &str,
    decoded: &DecodedPayload,
    merged: &BTreeMap<&str, &FieldSpec>,
) -> Result<(), RuntimeCheckError> {
    for (field_name, value) in decoded {
        if field_name == "type" {
            continue;
        }
        let spec = merged
            .get(field_name.as_str())
            .copied()
            .expect("unexpected fields were rejected earlier");
        check_field(event_type, field_name, spec, value)?;
    }
    Ok(())
}

fn check_field(
    event_type: &str,
    field_name: &str,
    spec: &FieldSpec,
    value: &FieldValue,
) -> Result<(), RuntimeCheckError> {
    match (&spec.ty, value) {
        (
            FieldType::Int8 | FieldType::Int16 | FieldType::Int32 | FieldType::Int64,
            FieldValue::Int(_) | FieldValue::UInt(_),
        )
        | (
            FieldType::Uint8 | FieldType::Uint16 | FieldType::Uint32 | FieldType::Uint64,
            FieldValue::UInt(_),
        )
        | (
            FieldType::Float32 | FieldType::Float64,
            FieldValue::Float(_) | FieldValue::Int(_) | FieldValue::UInt(_),
        ) => {
            let n = numeric_as_f64(value);
            check_numeric_range(event_type, field_name, &spec.ty, spec.range.as_ref(), n)?;
        }
        (
            FieldType::Uint8 | FieldType::Uint16 | FieldType::Uint32 | FieldType::Uint64,
            FieldValue::Int(i),
        ) => {
            // signed-positive を unsigned 型として受け入れる（負値は msgpack 上
            // 必ず Int になるため、ここで弾けば「unsigned 型に負値」を防げる）。
            if *i < 0 {
                return Err(RuntimeCheckError::TypeMismatch {
                    event_type: event_type.to_owned(),
                    field: field_name.to_owned(),
                    expected: ty_label(&spec.ty),
                    actual: value_label(value),
                });
            }
            #[allow(clippy::cast_precision_loss)]
            let n = *i as f64;
            check_numeric_range(event_type, field_name, &spec.ty, spec.range.as_ref(), n)?;
        }
        (FieldType::Bool, FieldValue::Bool(_)) => {}
        (FieldType::String, FieldValue::String(s)) => {
            check_max_length(event_type, field_name, spec.max_length, s.len() as u64)?;
        }
        (FieldType::Bytes, FieldValue::Bytes(b)) => {
            check_max_length(event_type, field_name, spec.max_length, b.len() as u64)?;
        }
        (FieldType::Enum, FieldValue::String(s)) => {
            check_enum_value(event_type, field_name, spec.values.as_deref(), s)?;
        }
        (FieldType::Array(inner_ty), FieldValue::Array(items)) => {
            check_max_length(event_type, field_name, spec.max_length, items.len() as u64)?;
            // 配列要素は `range` / `values` / `max_length` を持たない（events.yaml
            // の `array<T>` の T はスカラー語彙なので、要素検査は型と型のデフォルト値域だけで足りる）。
            let element_spec = FieldSpec {
                ty: (**inner_ty).clone(),
                range: None,
                values: None,
                max_length: None,
                optional: false,
                default: None,
            };
            for (index, element) in items.iter().enumerate() {
                let element_field = format!("{field_name}[{index}]");
                check_field(event_type, &element_field, &element_spec, element)?;
            }
        }
        _ => {
            return Err(RuntimeCheckError::TypeMismatch {
                event_type: event_type.to_owned(),
                field: field_name.to_owned(),
                expected: ty_label(&spec.ty),
                actual: value_label(value),
            });
        }
    }
    Ok(())
}

fn check_numeric_range(
    event_type: &str,
    field_name: &str,
    ty: &FieldType,
    range: Option<&RangeBound>,
    value: f64,
) -> Result<(), RuntimeCheckError> {
    let bound = range
        .and_then(|r| {
            let lo = yaml_to_f64(&r.min)?;
            let hi = yaml_to_f64(&r.max)?;
            if lo.is_finite() && hi.is_finite() && lo <= hi {
                Some((lo, hi))
            } else {
                None
            }
        })
        .or_else(|| ty.default_range());
    let Some((lo, hi)) = bound else {
        return Ok(());
    };
    if value.is_nan() || value < lo || value > hi {
        return Err(RuntimeCheckError::OutOfRange {
            event_type: event_type.to_owned(),
            field: field_name.to_owned(),
            value,
            lo,
            hi,
        });
    }
    Ok(())
}

fn check_max_length(
    event_type: &str,
    field_name: &str,
    max: Option<u64>,
    len: u64,
) -> Result<(), RuntimeCheckError> {
    let Some(max) = max else { return Ok(()) };
    if len > max {
        return Err(RuntimeCheckError::MaxLengthExceeded {
            event_type: event_type.to_owned(),
            field: field_name.to_owned(),
            len,
            max,
        });
    }
    Ok(())
}

fn check_enum_value(
    event_type: &str,
    field_name: &str,
    values: Option<&[String]>,
    actual: &str,
) -> Result<(), RuntimeCheckError> {
    // schema validator が enum に必ず values を要求するため None は schema 違反だが、
    // 防衛的に処理する（schema check 抜きで本層を直接呼ぶ caller の保護）。
    let Some(values) = values else { return Ok(()) };
    if values.iter().any(|v| v == actual) {
        return Ok(());
    }
    Err(RuntimeCheckError::EnumValueNotAllowed {
        event_type: event_type.to_owned(),
        field: field_name.to_owned(),
        value: actual.to_owned(),
    })
}

#[allow(clippy::cast_precision_loss)]
fn numeric_as_f64(value: &FieldValue) -> f64 {
    match value {
        FieldValue::Int(i) => *i as f64,
        FieldValue::UInt(u) => *u as f64,
        FieldValue::Float(f) => *f,
        _ => unreachable!("caller has matched only on numeric variants"),
    }
}

fn ty_label(ty: &FieldType) -> &'static str {
    match ty {
        FieldType::Int8 => "int8",
        FieldType::Uint8 => "uint8",
        FieldType::Int16 => "int16",
        FieldType::Uint16 => "uint16",
        FieldType::Int32 => "int32",
        FieldType::Uint32 => "uint32",
        FieldType::Int64 => "int64",
        FieldType::Uint64 => "uint64",
        FieldType::Float32 => "float32",
        FieldType::Float64 => "float64",
        FieldType::Bool => "bool",
        FieldType::String => "string",
        FieldType::Bytes => "bytes",
        FieldType::Enum => "enum",
        FieldType::Array(_) => "array",
    }
}

fn value_label(value: &FieldValue) -> &'static str {
    match value {
        FieldValue::Bool(_) => "bool",
        FieldValue::Int(_) => "int",
        FieldValue::UInt(_) => "uint",
        FieldValue::Float(_) => "float",
        FieldValue::String(_) => "string",
        FieldValue::Bytes(_) => "bytes",
        FieldValue::Array(_) => "array",
    }
}
