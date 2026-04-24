use serde::{Deserialize, Serialize};

use crate::pipeline::SignalSpecifier;
use crate::value::Value;

/// Direction of data flow through the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Input,
    Output,
}

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
}

/// Events streamed as JSON Lines from the runtime to the GUI over stdout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcEvent {
    /// Raw hardware event from a driver (Layer 1 / Layer 5).
    RawEvent {
        direction: Direction,
        driver: String,
        /// Opaque driver-specific payload (e.g. MIDI bytes, OSC path).
        payload: serde_json::Value,
    },

    /// Normalised component state after Layer 2 / Layer 4 processing.
    DeviceState {
        direction: Direction,
        device: String,
        specifier: SignalSpecifier,
        value: Value,
    },

    /// Mapper output signal (Layer 3).
    Signal {
        device: String,
        specifier: SignalSpecifier,
        value: Value,
    },

    /// Diagnostic log from any layer.
    Log {
        level: LogLevel,
        /// Which pipeline layer emitted this message.
        layer: String,
        device: Option<String>,
        message: String,
    },

    /// Highlights nodes/signals/components involved in an error propagation path.
    ErrorPath {
        nodes: Vec<String>,
        signals: Vec<SignalRef>,
        components: Vec<ComponentRef>,
    },
}

/// Reference to a signal in an error-path event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalRef {
    pub device: String,
    pub specifier: String,
}

/// Reference to a component in an error-path event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentRef {
    pub direction: Direction,
    pub device: String,
    pub specifier: SignalSpecifier,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_event_fields() {
        let e = IpcEvent::Log {
            level: LogLevel::Error,
            layer: "input-recognition".into(),
            device: Some("els03".into()),
            message: "unknown component".into(),
        };
        assert!(matches!(e, IpcEvent::Log { .. }));
    }

    #[test]
    fn device_state_dynamic_specifier() {
        let e = IpcEvent::DeviceState {
            direction: Direction::Input,
            device: "els03".into(),
            specifier: "upper.60.pressed".parse().unwrap(),
            value: Value::Bool(true),
        };
        assert!(matches!(e, IpcEvent::DeviceState { .. }));
    }

    #[test]
    fn ipc_event_json_roundtrip() {
        let e = IpcEvent::DeviceState {
            direction: Direction::Input,
            device: "els03".into(),
            specifier: "upper.60.pressed".parse().unwrap(),
            value: Value::Bool(true),
        };
        let json = serde_json::to_string(&e).unwrap();
        let decoded: IpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(e, decoded);
    }
}
