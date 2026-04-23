use crate::value::Value;

/// Identifies a single value field on a device component.
///
/// Format: `<component_id>.<value_name>` or `<component_id>.<note>.<value_name>` for keyboards.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignalSpecifier {
    pub component_id: String,
    /// MIDI note number; `Some` only for keyboard components.
    pub note: Option<u8>,
    pub value_name: String,
}

impl SignalSpecifier {
    pub fn new(component_id: impl Into<String>, value_name: impl Into<String>) -> Self {
        Self {
            component_id: component_id.into(),
            note: None,
            value_name: value_name.into(),
        }
    }

    pub fn with_note(
        component_id: impl Into<String>,
        note: u8,
        value_name: impl Into<String>,
    ) -> Self {
        Self {
            component_id: component_id.into(),
            note: Some(note),
            value_name: value_name.into(),
        }
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
    fn signal_specifier_keyboard() {
        let s = SignalSpecifier::with_note("upper", 60, "pressed");
        assert_eq!(s.note, Some(60));
        assert_eq!(s.value_name, "pressed");
    }

    #[test]
    fn component_state_fields() {
        let cs = ComponentState {
            device_id: "els03".into(),
            specifier: SignalSpecifier::new("expression", "value"),
            value: Value::Float(0.8),
        };
        assert_eq!(cs.device_id, "els03");
    }
}
