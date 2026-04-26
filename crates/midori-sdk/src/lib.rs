//! Driver SDK for the Midori signal bridge.
//!
//! Re-exports all public types from [`midori_core`] so driver authors can depend
//! on `midori-sdk` alone. The shared-memory SPSC ring buffer implementation
//! lives in this crate (the layout itself is defined in `midori_core::shm`).

pub mod driver;
pub mod spsc;

pub use midori_core as core;

pub use midori_core::ipc::*;
pub use midori_core::pipeline::*;
pub use midori_core::shm::*;
pub use midori_core::value::*;

pub use driver::{ControlCommand, DeviceEntry, Driver, DriverError, ProtocolError};
pub use spsc::{Consumer, Full, Producer, SpscStorage};

#[cfg(test)]
mod tests {
    use super::*;

    // ドライバー作者が midori_sdk::* だけで midori_core の型に到達できることを示す。
    #[test]
    fn it_should_expose_value_types_at_top_level() {
        let _: Value = Value::Bool(true);
        let _: ValueType = ValueType::Float;
        let _: OutOfRange = OutOfRange::default();
    }

    #[test]
    fn it_should_expose_pipeline_types_at_top_level() {
        let spec = SignalSpecifier::leaf("expression", "value");
        let _: ComponentState = ComponentState {
            device_id: "test".into(),
            specifier: spec.clone(),
            value: Value::Float(0.5),
        };
        let _: Signal = Signal {
            device_id: "test".into(),
            specifier: spec,
            value: Value::Float(0.5),
        };
    }

    #[test]
    fn it_should_expose_ipc_types_at_top_level() {
        let _: Direction = Direction::Input;
        let _: LogLevel = LogLevel::Info;
        let _: IpcEvent = IpcEvent::Log {
            level: LogLevel::Info,
            layer: "test".into(),
            device: None,
            message: "hello".into(),
        };
    }

    #[test]
    fn it_should_expose_shm_layout_at_top_level() {
        let _ = RING_CAPACITY;
        let _ = DEVICE_ID_MAX;
        let _ = SPECIFIER_MAX;
        let _: u8 = value_tag::PULSE;
    }
}
