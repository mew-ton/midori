use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::value::Value;

/// Identifies a value within a component via a dynamic dot-separated path.
///
/// The structure under `component_id` is component-type-dependent:
///
/// ```text
/// slider / knob:  component_id="expression"  path=["value"]
/// keyboard key:   component_id="upper"        path=["60", "pressed"]
/// hand tracking:  component_id="rightHand"    path=["index", "proximal", "bend"]
/// ```
///
/// Full string form: `<component_id>.<path[0]>.<path[1]>...`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignalSpecifier {
    pub component_id: String,
    /// Path segments beneath the component. Must contain at least one element (the leaf value name).
    pub path: Vec<String>,
}

/// Error returned by [`SignalSpecifier::try_new`] and [`str::parse`].
#[derive(Debug)]
pub struct SignalSpecifierError;

impl std::fmt::Display for SignalSpecifierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid signal specifier: expected '<component_id>.<path...>'"
        )
    }
}

impl std::error::Error for SignalSpecifierError {}

impl SignalSpecifier {
    /// Fallible constructor. Returns `Err` when `component_id`, `path`, or any path segment is empty.
    pub fn try_new(
        component_id: impl Into<String>,
        path: Vec<String>,
    ) -> Result<Self, SignalSpecifierError> {
        let component_id = component_id.into();
        if component_id.is_empty()
            || component_id.contains('.')
            || path.is_empty()
            || path.iter().any(|s| s.is_empty() || s.contains('.'))
        {
            return Err(SignalSpecifierError);
        }
        Ok(Self { component_id, path })
    }

    /// Panicking constructor for use-sites where the caller guarantees a non-empty path.
    /// Panics in debug builds; UB-free but unchecked in release.
    pub fn new(component_id: impl Into<String>, path: Vec<String>) -> Self {
        debug_assert!(!path.is_empty(), "SignalSpecifier path must not be empty");
        Self {
            component_id: component_id.into(),
            path,
        }
    }

    /// Convenience constructor for a single-segment path (e.g. slider `.value`).
    pub fn leaf(component_id: impl Into<String>, value_name: impl Into<String>) -> Self {
        Self::new(component_id, vec![value_name.into()])
    }
}

impl std::fmt::Display for SignalSpecifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.component_id, self.path.join("."))
    }
}

impl std::str::FromStr for SignalSpecifier {
    type Err = SignalSpecifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, '.');
        let component_id = parts.next().ok_or(SignalSpecifierError)?.to_owned();
        let rest = parts.next().ok_or(SignalSpecifierError)?;
        let path = rest.split('.').map(str::to_owned).collect();
        Self::try_new(component_id, path)
    }
}

impl Serialize for SignalSpecifier {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SignalSpecifier {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Normalised device event produced by Layer 2 and consumed by Layer 3 (mapper).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentState {
    pub device_id: String,
    pub specifier: SignalSpecifier,
    pub value: Value,
}

/// Mapper output produced by Layer 3 and consumed by Layer 4 (output recognition).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    pub device_id: String,
    pub specifier: SignalSpecifier,
    pub value: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn specifier_keyboard_key() {
        let s = SignalSpecifier::new("upper", vec!["60".into(), "pressed".into()]);
        assert_eq!(s.to_string(), "upper.60.pressed");
    }

    #[test]
    fn specifier_slider() {
        let s = SignalSpecifier::leaf("expression", "value");
        assert_eq!(s.to_string(), "expression.value");
    }

    #[test]
    fn specifier_hand_tracking() {
        let s = SignalSpecifier::new(
            "rightHand",
            vec!["index".into(), "proximal".into(), "bend".into()],
        );
        assert_eq!(s.to_string(), "rightHand.index.proximal.bend");
    }

    #[test]
    fn specifier_roundtrip() {
        let original = SignalSpecifier::new("upper", vec!["60".into(), "pressed".into()]);
        let parsed: SignalSpecifier = original.to_string().parse().unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn specifier_try_new_empty_path() {
        assert!(SignalSpecifier::try_new("upper", vec![]).is_err());
    }

    #[test]
    fn specifier_rejects_empty_component_id() {
        assert!("".parse::<SignalSpecifier>().is_err());
        assert!(SignalSpecifier::try_new("", vec!["pressed".into()]).is_err());
    }

    #[test]
    fn specifier_rejects_empty_segment() {
        assert!(".foo".parse::<SignalSpecifier>().is_err());
        assert!("a..b".parse::<SignalSpecifier>().is_err());
        assert!(SignalSpecifier::try_new("upper", vec!["60".into(), String::new()]).is_err());
    }

    #[test]
    fn specifier_rejects_dot_in_segment() {
        assert!(SignalSpecifier::try_new("up.per", vec!["pressed".into()]).is_err());
        assert!(SignalSpecifier::try_new("upper", vec!["60.pressed".into()]).is_err());
    }

    #[test]
    fn specifier_roundtrip_no_dot_in_segment() {
        let s = SignalSpecifier::new("upper", vec!["60".into(), "pressed".into()]);
        let parsed: SignalSpecifier = s.to_string().parse().unwrap();
        assert_eq!(s, parsed);
    }

    #[test]
    fn component_state_fields() {
        let cs = ComponentState {
            device_id: "els03".into(),
            specifier: SignalSpecifier::leaf("expression", "value"),
            value: Value::Float(0.8),
        };
        assert_eq!(cs.device_id, "els03");
    }
}
