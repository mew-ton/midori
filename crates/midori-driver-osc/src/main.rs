//! Official OSC driver binary for the Midori signal bridge.
//!
//! This is a skeleton: it wires the SDK's CLI scaffold (`<driver> list` /
//! `<driver> start`) to a `Driver` implementation that completes the
//! handshake and idles. The OSC protocol itself, device enumeration, and
//! actual signal forwarding are deliberately deferred to follow-up work
//! and live behind no-op handlers here.
//!
//! # What this binary currently does
//!
//! - `list`: prints an empty device array (`[]`). Device enumeration is
//!   intentionally not implemented yet — the bridge treats an empty list
//!   as "this driver is wired up but has no devices to offer right now",
//!   which is the correct state for a skeleton.
//! - `start`: emits the SDK `hello` message, waits for `hello_ack`, then
//!   accepts and silently drops any control commands (`connect`,
//!   `configure`, `disconnect`) until stdin EOF or SIGTERM/SIGINT.
//!
//! Returning `Ok(())` from every command handler keeps the bridge's
//! lifecycle tests against this driver passing: the bridge sees a clean
//! handshake, can issue commands without errors, and observes a graceful
//! exit on shutdown. When real OSC support lands, the handler bodies are
//! the place to dispatch into the protocol implementation.

use std::process::ExitCode;

use midori_sdk::{ControlCommand, DeviceEntry, Driver, DriverError};

/// Skeleton driver state. Holds no resources today; future OSC work will
/// add a UDP socket handle, a worker thread, and an SPSC `Producer` here.
struct OscDriver;

impl Driver for OscDriver {
    fn list_devices(&mut self) -> Vec<DeviceEntry> {
        // Device discovery (enumerating reachable OSC endpoints) is out of
        // scope for the skeleton. Returning an empty list is the documented
        // way to say "no devices available" without erroring out.
        Vec::new()
    }

    fn handle_command(&mut self, _command: ControlCommand) -> Result<(), DriverError> {
        // Accept and drop. Returning Ok keeps the SDK's command loop alive
        // so the bridge can drive the full lifecycle (connect → configure →
        // disconnect) against this skeleton without surfacing spurious
        // protocol errors.
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), DriverError> {
        // No resources to release yet. Real implementations will tear down
        // the OSC socket and join worker threads here.
        Ok(())
    }
}

fn main() -> ExitCode {
    midori_sdk::driver::run(OscDriver)
}
