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

impl SignalSpecifier {
    /// Build from a component id and path segments. `path` must be non-empty.
    pub fn new(component_id: impl Into<String>, path: Vec<String>) -> Self {
        assert!(!path.is_empty(), "SignalSpecifier path must not be empty");
        Self {
            component_id: component_id.into(),
            path,
        }
    }

    /// Convenience constructor for a single-segment path (e.g. slider `.value`).
    pub fn leaf(component_id: impl Into<String>, value_name: impl Into<String>) -> Self {
        Self::new(component_id, vec![value_name.into()])
    }

    /// Returns the full dot-separated string representation.
    #[must_use]
    pub fn to_dot_string(&self) -> String {
        format!("{}.{}", self.component_id, self.path.join("."))
    }
}

/// Normalised device event produced by Layer 2 and consumed by Layer 3 (mapper).
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentState {
    pub device_id: String,
    pub specifier: SignalSpecifier,
    pub value: Value,
}

/// Mapper output produced by Layer 3 and consumed by Layer 4 (output recognition).
#[derive(Debug, Clone, PartialEq)]
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
        assert_eq!(s.to_dot_string(), "upper.60.pressed");
    }

    #[test]
    fn specifier_slider() {
        let s = SignalSpecifier::leaf("expression", "value");
        assert_eq!(s.to_dot_string(), "expression.value");
    }

    #[test]
    fn specifier_hand_tracking() {
        let s = SignalSpecifier::new(
            "rightHand",
            vec!["index".into(), "proximal".into(), "bend".into()],
        );
        assert_eq!(s.to_dot_string(), "rightHand.index.proximal.bend");
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
