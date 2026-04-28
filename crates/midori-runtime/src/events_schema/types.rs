//! events.yaml の Rust 型表現と serde 周辺ヘルパ。

use std::collections::BTreeMap;

use serde::Deserialize;

/// Top-level structure of `events.yaml`.
///
/// `defaults` は spec 上「全イベント共通フィールドのデフォルト宣言」として
/// 扱う。本書（loader / validator）は **defaults の field spec 単独を検証
/// する** が、各 event への merge は行わない。merge を必要とする呼び出し側
/// （Bridge runtime 経路の確定後に実装）が自分で扱う。
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EventsSchema {
    pub schema_version: u32,
    #[serde(deserialize_with = "deserialize_unique_event_map")]
    pub events: BTreeMap<String, EventDef>,
    #[serde(default, deserialize_with = "deserialize_unique_field_map")]
    pub defaults: BTreeMap<String, FieldSpec>,
}

/// Per-event definition.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EventDef {
    #[serde(deserialize_with = "deserialize_unique_field_map")]
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
#[serde(deny_unknown_fields)]
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
            let inner_ty = Self::parse(inner.trim())?;
            // spec restricts `array<T>` の T をスカラー型 table の値に限定
            // （`array<enum>` や `array<array<...>>` は語彙外）
            if !inner_ty.is_scalar() {
                return None;
            }
            return Some(Self::Array(Box::new(inner_ty)));
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
    pub(super) fn supports_range(&self) -> bool {
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
    pub(super) fn supports_max_length(&self) -> bool {
        matches!(self, Self::String | Self::Bytes | Self::Array(_))
    }

    /// Whether the type can be referenced by `binding_filter`.
    /// Variable-length / structured types are excluded.
    pub(super) fn is_filterable(&self) -> bool {
        !matches!(self, Self::Bytes | Self::Array(_))
    }

    /// Whether the type belongs to the スカラー型 table in spec
    /// (`array<T>` の T として許可される範囲)。
    fn is_scalar(&self) -> bool {
        !matches!(self, Self::Enum | Self::Array(_))
    }

    /// Default integer/float bounds (inclusive). `None` for non-numeric types.
    /// 整数の `f64` 化は schema validator の境界比較用なので、`i64::MAX`
    /// 付近で 1 ulp ずれても実害がない（events.yaml で `int64` ぴったり
    /// 境界の `range` を書く driver は事実上いない）と判断して許容する。
    #[allow(clippy::cast_precision_loss)]
    pub(super) fn default_range(&self) -> Option<(f64, f64)> {
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

/// 同名キーで silently 上書きせず、duplicate を起動時エラーで弾くための
/// 汎用 deserializer。spec は events / fields / defaults の重複キーを
/// すべて起動時エラー扱いとしているため、対象 map の値型ごとに薄い
/// wrapper 関数を用意してこれを呼ぶ。
fn deserialize_unique_map<'de, V, D>(
    deserializer: D,
    expecting: &'static str,
    duplicate_label: &'static str,
) -> Result<BTreeMap<String, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    V: Deserialize<'de>,
{
    struct UniqueMap<V>(&'static str, &'static str, std::marker::PhantomData<V>);
    impl<'de, V> serde::de::Visitor<'de> for UniqueMap<V>
    where
        V: Deserialize<'de>,
    {
        type Value = BTreeMap<String, V>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, V>()? {
                if result.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate {}: {}",
                        self.1, key
                    )));
                }
                result.insert(key, value);
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(UniqueMap::<V>(
        expecting,
        duplicate_label,
        std::marker::PhantomData,
    ))
}

fn deserialize_unique_event_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, EventDef>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_unique_map(
        deserializer,
        "a map of event names to event definitions",
        "event name",
    )
}

fn deserialize_unique_field_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, FieldSpec>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_unique_map(
        deserializer,
        "a map of field names to field specs",
        "field name",
    )
}
