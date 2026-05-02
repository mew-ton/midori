//! Test fixture driver binary used by Bridge spawn / handshake tests.
//!
//! This binary speaks the driver-side half of the JSON Lines control protocol
//! described in `design/10-driver-plugin.md`「通信アーキテクチャ」「バージョン互換性」.
//! Its only job is to be deterministic enough that the Bridge's `spawn_driver`
//! flow can be exercised in three regimes:
//!
//! - default: emit a valid `hello` and then enter the stdin `BufRead`
//!   scaffold loop (the loop body is intentionally a no-op; subtask (b)
//!   will plug in handlers for `connect` / `disconnect` / `configure`).
//! - `--no-hello`: never emit `hello`. Used to exercise the Bridge's
//!   handshake timeout path.
//! - `--bad-version`: emit `hello` with an SDK major that the Bridge
//!   rejects. Used to exercise the `IncompatibleSdk` path.
//!
//! The binary only ever exercises the `start` subcommand for the spawn /
//! handshake test surface; the argument parser is structured so the `list`
//! subcommand can be added later without reshaping the dispatcher.

use std::io::{BufRead, BufReader, Write};
use std::process::ExitCode;

/// CLI flag that suppresses the hello emission entirely. The driver still
/// enters the stdin read loop so the parent can observe stdin EOF on its own
/// terms (the timeout test relies on the parent giving up first).
const FLAG_NO_HELLO: &str = "--no-hello";

/// CLI flag that emits a `hello` whose `sdk_version` reports a major that the
/// Bridge will treat as incompatible. The exact value lives next to the
/// Bridge's compatibility check so the two stay coupled by intent: see
/// `BAD_SDK_VERSION` in `crates/midori-runtime/src/driver_proc.rs`.
const FLAG_BAD_VERSION: &str = "--bad-version";

/// SDK version reported by the default fixture path. Held as a constant so
/// the test fixture and the Bridge-side compatibility check remain in lockstep.
const DEFAULT_SDK_VERSION: &str = "0.1.0";

/// SDK version reported by the `--bad-version` fixture path. Chosen to land
/// outside the Bridge's accepted-major set so the Bridge replies with
/// `hello_ack { compatible: false }` and surfaces `IncompatibleSdk`.
const BAD_SDK_VERSION: &str = "99.0.0";

fn main() -> ExitCode {
    let mut args = std::env::args();
    let _bin = args.next();
    let subcommand = args.next();
    match subcommand.as_deref() {
        Some("start") => exec_start(args.collect::<Vec<_>>().as_slice()),
        Some(other) => {
            eprintln!("midori-driver-dummy: unknown subcommand: {other}");
            ExitCode::FAILURE
        }
        None => {
            eprintln!(
                "midori-driver-dummy: usage: midori-driver-dummy start [--no-hello|--bad-version]"
            );
            ExitCode::FAILURE
        }
    }
}

fn exec_start(flags: &[String]) -> ExitCode {
    let mut emit_hello = true;
    let mut sdk_version = DEFAULT_SDK_VERSION;
    for flag in flags {
        match flag.as_str() {
            FLAG_NO_HELLO => emit_hello = false,
            FLAG_BAD_VERSION => sdk_version = BAD_SDK_VERSION,
            other => {
                eprintln!("midori-driver-dummy: unknown flag: {other}");
                return ExitCode::FAILURE;
            }
        }
    }

    if emit_hello {
        if let Err(err) = emit_hello_line(sdk_version) {
            eprintln!("midori-driver-dummy: failed to write hello: {err}");
            return ExitCode::FAILURE;
        }
    }

    // Stdin BufRead scaffold loop. Subtask (b) will replace the no-op body
    // with `match` arms for `hello_ack` / `connect` / `disconnect` /
    // `configure`. Today the loop just drains stdin until EOF, which is the
    // signal the parent uses to terminate the child gracefully.
    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());
    for line in reader.lines() {
        match line {
            Ok(_line) => {
                // Subtask (b) hook point: dispatch on parsed message type.
            }
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
