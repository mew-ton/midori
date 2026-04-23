use crate::value::Value;

/// Direction of data flow through the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
}

/// Events streamed as JSON Lines from the runtime to the GUI over stdout.
#[derive(Debug, Clone, PartialEq)]
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
        component: String,
        /// Present for keyboard components.
        note: Option<u8>,
        value_name: String,
        value: Value,
    },

    /// Mapper output signal (Layer 3).
    Signal {
        device: String,
        /// Signal specifier string, e.g. `"upper.60.pressed"`.
        name: String,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRef {
    pub device: String,
    pub name: String,
}

/// Reference to a component in an error-path event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRef {
    pub direction: Direction,
    pub device: String,
    pub component: String,
    pub note: Option<u8>,
    pub value_name: String,
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
}
