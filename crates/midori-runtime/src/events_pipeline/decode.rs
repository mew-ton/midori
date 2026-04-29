//! Inline payload (msgpack) → 動的 [`DecodedPayload`] への decode 層。
//!
//! 本層は「msgpack の bytes を events.yaml schema が扱える語彙へ落とすだけ」
//! の責務に絞る。schema 違反（type 不一致 / range 外 / 必須欠落）の判定は
//! 上位の `runtime_check` モジュールで行う。

use std::collections::BTreeMap;

use rmpv::Value;

/// events.yaml の語彙に合わせた decode 後の値表現。
///
/// 整数は msgpack 表現に従い signed / unsigned を分離して保持する。schema
/// 側の `int*` / `uint*` 型と照合する際に、上位層が必要な変換を行う。
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<FieldValue>),
}

/// 1 イベントの decode 結果。msgpack map のキー → 値。
pub type DecodedPayload = BTreeMap<String, FieldValue>;

/// Decode 失敗の原因。
#[derive(Debug)]
pub enum DecodeError {
    /// msgpack 自体の解析失敗（不完全 / 破損バイト列）。
    Parse(rmpv::decode::Error),
    /// トップレベルが map ではない（events.yaml は map of fields のみ許容）。
    NotAMap,
    /// 余剰バイトが残っている（1 payload に複数 msgpack value が連結されている）。
    TrailingBytes,
    /// map のキーが文字列ではない。
    NonStringKey,
    /// UTF-8 として無効な msgpack str。
    NonUtf8String { path: String },
    /// events.yaml の語彙に存在しない msgpack type（nil / ext）が現れた。
    UnsupportedValue { path: String, kind: &'static str },
    /// map / nil / ext がフィールド値としてネストしている。events.yaml は
    /// scalar / `bytes` / `string` / `array<scalar>` のみを許容する。
    NestedMap { path: String },
    /// 配列要素が array や map（= 非 scalar）。`array<scalar>` の制約に反する。
    NonScalarArrayElement { path: String },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(source) => write!(f, "msgpack parse failed: {source}"),
            Self::NotAMap => f.write_str("top-level value must be a msgpack map"),
            Self::TrailingBytes => {
                f.write_str("payload contains trailing bytes after the top-level map")
            }
            Self::NonStringKey => f.write_str("map keys must be strings"),
            Self::NonUtf8String { path } => write!(f, "{path}: string is not valid UTF-8"),
            Self::UnsupportedValue { path, kind } => write!(
                f,
                "{path}: msgpack value of kind `{kind}` is not in the events.yaml vocabulary"
            ),
            Self::NestedMap { path } => write!(f, "{path}: nested maps are not allowed"),
            Self::NonScalarArrayElement { path } => {
                write!(f, "{path}: array elements must be scalar values")
            }
        }
    }
}

impl std::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(source) => Some(source),
            _ => None,
        }
    }
}

/// 1 inline payload（msgpack バイト列）を decode する。
///
/// 戻り値は msgpack map のキー → 値の `BTreeMap`。rmpv は重複キーを
/// `Vec<(Value, Value)>` のまま返すが、本層は `BTreeMap::insert` の
/// 後勝ちで暗黙にマージする（msgpack 規格上重複キーは未定義動作で、
/// driver SDK が dict / struct から encode する正常経路では発生しない
/// ため、防衛的検出までは行わない）。重複検出が必要になったらここで
/// 早期 reject に切り替える。
pub fn decode_event(bytes: &[u8]) -> Result<DecodedPayload, DecodeError> {
    let mut cursor: &[u8] = bytes;
    let value = rmpv::decode::read_value(&mut cursor).map_err(DecodeError::Parse)?;
    if !cursor.is_empty() {
        return Err(DecodeError::TrailingBytes);
    }
    let Value::Map(entries) = value else {
        return Err(DecodeError::NotAMap);
    };
    let mut out = BTreeMap::new();
    for (key_value, raw) in entries {
        let key = key_value
            .as_str()
            .ok_or(DecodeError::NonStringKey)?
            .to_owned();
        let field = convert_value(&key, raw)?;
        out.insert(key, field);
    }
    Ok(out)
}

fn convert_value(path: &str, value: Value) -> Result<FieldValue, DecodeError> {
    Ok(match value {
        Value::Boolean(b) => FieldValue::Bool(b),
        Value::Integer(i) => {
            if let Some(u) = i.as_u64() {
                FieldValue::UInt(u)
            } else if let Some(s) = i.as_i64() {
                FieldValue::Int(s)
            } else {
                // msgpack int の規格上 [-2^63, 2^64-1] の範囲しか表現できないため、
                // as_u64 と as_i64 が両方 None になる入力は rmpv 側で構築不能。
                // 防衛的に reject。
                return Err(DecodeError::UnsupportedValue {
                    path: path.to_owned(),
                    kind: "integer-out-of-range",
                });
            }
        }
        Value::F32(v) => FieldValue::Float(f64::from(v)),
        Value::F64(v) => FieldValue::Float(v),
        Value::String(s) => match s.into_str() {
            Some(text) => FieldValue::String(text),
            None => {
                return Err(DecodeError::NonUtf8String {
                    path: path.to_owned(),
                });
            }
        },
        Value::Binary(bytes) => FieldValue::Bytes(bytes),
        Value::Array(items) => {
            let mut elements = Vec::with_capacity(items.len());
            for (index, item) in items.into_iter().enumerate() {
                let elem_path = format!("{path}[{index}]");
                let elem = convert_value(&elem_path, item)?;
                if matches!(elem, FieldValue::Array(_)) {
                    return Err(DecodeError::NonScalarArrayElement { path: elem_path });
                }
                elements.push(elem);
            }
            FieldValue::Array(elements)
        }
        Value::Map(_) => {
            return Err(DecodeError::NestedMap {
                path: path.to_owned(),
            });
        }
        Value::Nil => {
            return Err(DecodeError::UnsupportedValue {
                path: path.to_owned(),
                kind: "nil",
            });
        }
        Value::Ext(_, _) => {
            return Err(DecodeError::UnsupportedValue {
                path: path.to_owned(),
                kind: "ext",
            });
        }
    })
}
