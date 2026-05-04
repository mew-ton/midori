//! Official MIDI driver binary for the Midori signal bridge.
//!
//! This is a skeleton: it wires the SDK's CLI scaffold (`<driver> list` /
//! `<driver> start`) to a `Driver` implementation that enumerates MIDI
//! input ports via `midir` and parks all command handlers as no-ops.
//! Actual MIDI message reception, connection lifecycle management, and
//! event emission are deferred to follow-up work and live behind no-op
//! handlers here.
//!
//! # What this binary currently does
//!
//! - `list`: enumerates MIDI input ports via `midir::MidiInput` and prints
//!   a JSON array of `DeviceEntry { value, label }`. `value` is the stable
//!   `MidiInputPort::id()` (the key the bridge passes back through
//!   `Connect`); `label` is the human-readable `port_name()`. Ports whose
//!   name lookup fails are silently skipped. If `MidiInput::new` itself
//!   fails, an empty array is returned — the bridge treats this as "no
//!   devices to offer" rather than as a hard error.
//! - `start`: emits the SDK `hello` message, waits for `hello_ack`, then
//!   accepts and silently drops any control commands (`connect`,
//!   `configure`, `disconnect`) until stdin EOF or SIGTERM/SIGINT.
//!
//! Returning `Ok(())` from every command handler keeps the bridge's
//! lifecycle tests against this driver passing: the bridge sees a clean
//! handshake, can issue commands without errors, and observes a graceful
//! exit on shutdown. When real MIDI support lands, the handler bodies are
//! the place to spawn the input thread, dispatch incoming events through
//! the SDK SPSC ring, and tear down resources on shutdown.

use midir::MidiInput;
use std::process::ExitCode;

use midori_sdk::{ControlCommand, DeviceEntry, Driver, DriverError};

/// Enumerate available MIDI input ports through `midir`.
///
/// Returns an empty `Vec` if `MidiInput::new` fails (treated as "no
/// devices available" rather than surfaced to the bridge as an error).
/// Ports whose `port_name()` lookup fails are silently skipped — the
/// listing reports only ports that can be addressed by both stable id
/// and human-readable label.
fn collect_devices() -> Vec<DeviceEntry> {
    let Ok(midi_in) = MidiInput::new("midori-driver-midi") else {
        return Vec::new();
    };

    midi_in
        .ports()
        .iter()
        .filter_map(|port| {
            Some(DeviceEntry {
                value: port.id(),
                label: midi_in.port_name(port).ok()?,
            })
        })
        .collect()
}

/// Skeleton driver state. Holds no resources today; future MIDI work will
/// add a `midir::MidiInput` client, an `Option<MidiInputConnection<...>>`,
/// and an SPSC `Producer` here.
struct MidiDriver;

impl Driver for MidiDriver {
    fn list_devices(&mut self) -> Vec<DeviceEntry> {
        collect_devices()
    }

    fn handle_command(&mut self, _command: ControlCommand) -> Result<(), DriverError> {
        // Accept and drop. Returning Ok keeps the SDK's command loop alive
        // so the bridge can drive the full lifecycle (connect → configure →
        // disconnect) against this skeleton without surfacing spurious
        // protocol errors.
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), DriverError> {
        // No resources to release yet. Real implementations will close the
        // midir connection and join worker threads here.
        Ok(())
    }
}

fn main() -> ExitCode {
    midori_sdk::driver::run(MidiDriver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use midori_runtime::events_schema::{validate, EventsSchema};

    const EVENTS_YAML: &str = include_str!("../events.yaml");

    #[test]
    fn it_should_load_events_yaml_as_valid_schema() {
        let schema: EventsSchema = serde_yaml_ng::from_str(EVENTS_YAML)
            .expect("events.yaml must deserialize as EventsSchema");

        validate(&schema).expect("events.yaml must pass schema validation");
    }

    #[test]
    fn it_should_return_device_list_without_panic() {
        // Hardware-less environment may return an empty Vec.
        // Goal: ensure collect_devices() does not panic.
        let _devices = collect_devices();
    }
}
