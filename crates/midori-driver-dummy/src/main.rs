//! Test fixture driver binary used by Bridge spawn / handshake tests.
//!
//! This binary speaks the driver-side half of the JSON Lines control protocol
//! described in `design/10-driver-plugin.md`「通信アーキテクチャ」「バージョン互換性」.
//! Three compiled binaries share this source file, each selecting a fixture
//! mode via the build-time `CARGO_BIN_NAME`:
//!
//! - `midori-driver-dummy`: emit a valid `hello` and drain stdin until EOF.
//!   The drain loop is a deliberate no-op so a future change can slot a
//!   message dispatcher in without restructuring the binary.
//! - `midori-driver-dummy-no-hello`: never emit `hello`. Used to exercise
//!   the Bridge's handshake-timeout path.
//! - `midori-driver-dummy-bad-version`: emit a `hello` whose `sdk_version`
//!   reports a major outside the Bridge's accepted-major set (defined as
//!   `ACCEPTED_SDK_MAJORS` in the runtime crate's `driver_proc` module).
//!   Used to exercise the `IncompatibleSdk` path.
//!
//! Selecting fixture mode by binary name (rather than by CLI flag) lets the
//! integration test hand `spawn_driver` an exact, pre-built binary path —
//! avoiding any need to materialise a per-test wrapper script, which races
//! against parallel tests on Linux (ETXTBSY at exec).
//!
//! The binary only ever exercises the `start` subcommand; the argument
//! parser is structured so the `list` subcommand can be added later
//! without reshaping the dispatcher.

use std::io::{BufRead, BufReader, Write};
use std::process::ExitCode;

/// Fixture mode resolved from the binary's compiled-in name. The runtime
/// fall-through to `Success` is just for forward-compatibility — every
/// binary actually shipped here matches one of the three known names.
#[derive(Debug, Clone, Copy)]
enum FixtureMode {
    Success,
    NoHello,
    BadVersion,
}

const fn fixture_mode_for(bin_name: &str) -> FixtureMode {
    let bytes = bin_name.as_bytes();
    if matches_const(bytes, b"midori-driver-dummy-no-hello") {
        FixtureMode::NoHello
    } else if matches_const(bytes, b"midori-driver-dummy-bad-version") {
        FixtureMode::BadVersion
    } else {
        FixtureMode::Success
    }
}

const fn matches_const(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut i = 0;
    while i < left.len() {
        if left[i] != right[i] {
            return false;
        }
        i += 1;
    }
    true
}

const FIXTURE: FixtureMode = fixture_mode_for(env!("CARGO_BIN_NAME"));

/// SDK version reported by the success path. Held as a constant so the
/// fixture and the Bridge-side compatibility check remain in lockstep.
const DEFAULT_SDK_VERSION: &str = "0.1.0";

/// SDK version reported by the bad-version fixture. Chosen to land outside
/// the Bridge's accepted-major set so the Bridge replies with
/// `hello_ack { compatible: false }` and surfaces `IncompatibleSdk`.
const BAD_SDK_VERSION: &str = "99.0.0";

fn main() -> ExitCode {
    let mut args = std::env::args();
    let _bin = args.next();
    let subcommand = args.next();
    match subcommand.as_deref() {
        Some("start") => exec_start(),
        Some(other) => {
            eprintln!("midori-driver-dummy: unknown subcommand: {other}");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("midori-driver-dummy: usage: midori-driver-dummy start");
            ExitCode::FAILURE
        }
    }
}

fn exec_start() -> ExitCode {
    match FIXTURE {
        FixtureMode::Success => {
            if let Err(err) = emit_hello_line(DEFAULT_SDK_VERSION) {
                eprintln!("midori-driver-dummy: failed to write hello: {err}");
                return ExitCode::FAILURE;
            }
        }
        FixtureMode::NoHello => {
            // Skip hello emission; the test relies on the Bridge timing out.
        }
        FixtureMode::BadVersion => {
            if let Err(err) = emit_hello_line(BAD_SDK_VERSION) {
                eprintln!("midori-driver-dummy: failed to write hello: {err}");
                return ExitCode::FAILURE;
            }
        }
    }

    // Drain stdin until EOF. The loop body is a deliberate no-op: the parent
    // closes stdin when it wants the child gone, and the BufRead structure
    // is the slot a future change can fill with a message dispatcher
    // without reshaping the binary.
    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());
    for line in reader.lines() {
        match line {
            Ok(_line) => {}
            Err(err) => {
                eprintln!("midori-driver-dummy: stdin read error: {err}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

fn emit_hello_line(sdk_version: &str) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let payload = serde_json::json!({
        "type": "hello",
        "sdk_version": sdk_version,
    });
    serde_json::to_writer(&mut out, &payload)?;
    out.write_all(b"\n")?;
    out.flush()
}
