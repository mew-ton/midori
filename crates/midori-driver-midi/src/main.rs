//! Official MIDI driver binary for the Midori signal bridge.

use midir::MidiInput;
use std::process::ExitCode;

use midori_sdk::{ControlCommand, DeviceEntry, Driver, DriverError};

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

struct MidiDriver;

impl Driver for MidiDriver {
    fn list_devices(&mut self) -> Vec<DeviceEntry> {
        collect_devices()
    }

    fn handle_command(&mut self, _command: ControlCommand) -> Result<(), DriverError> {
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), DriverError> {
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
