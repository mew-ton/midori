//! Test fixture driver binary used by Bridge spawn / handshake / lifecycle
//! tests.
//!
//! This binary speaks the driver-side half of the JSON Lines control protocol
//! described in `design/10-driver-plugin.md`「通信アーキテクチャ」「バージョン互換性」.
//! Multiple compiled binaries share this source file, each selecting a
//! fixture mode via the build-time `CARGO_BIN_NAME`:
//!
//! Handshake fixtures:
//! - `midori-driver-dummy`: emit a valid `hello`, accept any control
//!   messages on stdin (each is acknowledged on stderr as a no-op), and
//!   exit on stdin EOF.
//! - `midori-driver-dummy-no-hello`: never emit `hello`. Exercises the
//!   Bridge's handshake-timeout path.
//! - `midori-driver-dummy-bad-version`: emit a `hello` whose `sdk_version`
//!   reports a major outside the Bridge's accepted-major set. Exercises
//!   the `IncompatibleSdk` path.
//!
//! Lifecycle / log-forwarding fixtures:
//! - `midori-driver-dummy-noisy-stdout`: emit `hello`, then write a
//!   non-JSON line to stdout, then idle until stdin EOF / SIGTERM.
//!   Exercises the bridge's "non-JSON stdout → log forwarder" path.
//! - `midori-driver-dummy-sigterm-graceful`: emit `hello`, install a
//!   SIGTERM handler that sets a shutdown flag, and poll the flag in the
//!   stdin loop so SIGTERM produces a clean exit.
//! - `midori-driver-dummy-sigterm-ignore`: emit `hello`, install a
//!   SIGTERM handler that ignores the signal entirely. Forces the bridge's
//!   SIGKILL escalation path. Also ignores stdin EOF so the only way out
//!   is the SIGKILL the bridge sends after its grace period.
//! - `midori-driver-dummy-exit-error`: emit `hello`, then exit with status
//!   1 immediately. Exercises the bridge's abnormal-exit logging.
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
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::sync::Arc;
use std::time::Duration;

/// Fixture mode resolved from the binary's compiled-in name. The runtime
/// fall-through to `Success` is just for forward-compatibility — every
/// binary actually shipped here matches one of the known names.
#[derive(Debug, Clone, Copy)]
enum FixtureMode {
    Success,
    NoHello,
    BadVersion,
    NoisyStdout,
    SigtermGraceful,
    SigtermIgnore,
    ExitError,
}

const fn fixture_mode_for(bin_name: &str) -> FixtureMode {
    let bytes = bin_name.as_bytes();
    if matches_const(bytes, b"midori-driver-dummy-no-hello") {
        FixtureMode::NoHello
    } else if matches_const(bytes, b"midori-driver-dummy-bad-version") {
        FixtureMode::BadVersion
    } else if matches_const(bytes, b"midori-driver-dummy-noisy-stdout") {
        FixtureMode::NoisyStdout
    } else if matches_const(bytes, b"midori-driver-dummy-sigterm-graceful") {
        FixtureMode::SigtermGraceful
    } else if matches_const(bytes, b"midori-driver-dummy-sigterm-ignore") {
        FixtureMode::SigtermIgnore
    } else if matches_const(bytes, b"midori-driver-dummy-exit-error") {
        FixtureMode::ExitError
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

/// Polling interval used by the SIGTERM-graceful fixture's stdin loop. Short
/// enough that the test does not have to budget seconds of wall-clock to
/// observe a clean exit; long enough that the loop is not a busy-wait.
const SIGTERM_POLL_INTERVAL: Duration = Duration::from_millis(20);

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
        FixtureMode::Success => run_success(),
        FixtureMode::NoHello => run_no_hello(),
        FixtureMode::BadVersion => run_bad_version(),
        FixtureMode::NoisyStdout => run_noisy_stdout(),
        FixtureMode::SigtermGraceful => run_sigterm_graceful(),
        FixtureMode::SigtermIgnore => run_sigterm_ignore(),
        FixtureMode::ExitError => run_exit_error(),
    }
}

fn run_success() -> ExitCode {
    if let Err(err) = emit_hello_line(DEFAULT_SDK_VERSION) {
        eprintln!("midori-driver-dummy: failed to write hello: {err}");
        return ExitCode::FAILURE;
    }
    drain_stdin_until_eof()
}

fn run_no_hello() -> ExitCode {
    // Skip hello emission; the test relies on the Bridge timing out.
    // Idle on stdin so we do not exit before the bridge's timeout fires.
    drain_stdin_until_eof()
}

fn run_bad_version() -> ExitCode {
    if let Err(err) = emit_hello_line(BAD_SDK_VERSION) {
        eprintln!("midori-driver-dummy: failed to write hello: {err}");
        return ExitCode::FAILURE;
    }
    drain_stdin_until_eof()
}

fn run_noisy_stdout() -> ExitCode {
    if let Err(err) = emit_hello_line(DEFAULT_SDK_VERSION) {
        eprintln!("midori-driver-dummy: failed to write hello: {err}");
        return ExitCode::FAILURE;
    }
    // Emit a line that is **not** valid JSON. The bridge's log forwarder
    // wraps this into a `type=log layer=driver/<name>` entry.
    if let Err(err) = emit_raw_stdout_line("midori-driver-dummy: noisy debug line") {
        eprintln!("midori-driver-dummy: failed to write noisy line: {err}");
        return ExitCode::FAILURE;
    }
    drain_stdin_until_eof()
}

fn run_sigterm_graceful() -> ExitCode {
    if let Err(err) = emit_hello_line(DEFAULT_SDK_VERSION) {
        eprintln!("midori-driver-dummy: failed to write hello: {err}");
        return ExitCode::FAILURE;
    }
    #[cfg(unix)]
    {
        let shutdown = Arc::new(AtomicBool::new(false));
        if let Err(err) = install_sigterm_flag(&shutdown) {
            eprintln!("midori-driver-dummy: failed to install SIGTERM handler: {err}");
            return ExitCode::FAILURE;
        }
        // Poll the flag in a tight-but-bounded loop. The bridge's `Drop`
        // sends SIGTERM, this fixture flips the flag, the next iteration
        // exits cleanly. Stdin is intentionally not consumed — the test
        // checks the SIGTERM path, not the stdin-EOF path.
        while !shutdown.load(Ordering::SeqCst) {
            std::thread::sleep(SIGTERM_POLL_INTERVAL);
        }
        ExitCode::SUCCESS
    }
    #[cfg(not(unix))]
    {
        // Windows: just drain stdin. The bridge's SIGKILL fallback (via
        // `Child::kill()` → `TerminateProcess`) is the test's expectation
        // there; the parent issue treats Windows lifecycle as out of scope.
        drain_stdin_until_eof()
    }
}

fn run_sigterm_ignore() -> ExitCode {
    if let Err(err) = emit_hello_line(DEFAULT_SDK_VERSION) {
        eprintln!("midori-driver-dummy: failed to write hello: {err}");
        return ExitCode::FAILURE;
    }
    #[cfg(unix)]
    {
        if let Err(err) = install_sigterm_ignore() {
            eprintln!("midori-driver-dummy: failed to ignore SIGTERM: {err}");
            return ExitCode::FAILURE;
        }
        // Sleep forever. Stdin EOF is also ignored — the bridge must use
        // SIGKILL to terminate. SIGKILL bypasses signal handlers and Rust's
        // process exits with the default signal-termination status.
        loop {
            std::thread::sleep(Duration::from_mins(1));
        }
    }
    #[cfg(not(unix))]
    {
        loop {
            std::thread::sleep(Duration::from_mins(1));
        }
    }
}

fn run_exit_error() -> ExitCode {
    if let Err(err) = emit_hello_line(DEFAULT_SDK_VERSION) {
        eprintln!("midori-driver-dummy: failed to write hello: {err}");
        return ExitCode::FAILURE;
    }
    // Wait for the bridge's hello_ack on stdin before exiting. Without this
    // wait, the child would frequently exit before the bridge finishes
    // writing hello_ack, surfacing as `HelloAckWrite(BrokenPipe)` on the
    // bridge side and masking the abnormal-exit code path under test.
    // Reading exactly one line is enough — we do not interpret the ack
    // content, only synchronize on its arrival.
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();
    let _ = handle.read_line(&mut line);
    drop(handle);
    ExitCode::from(1)
}

/// Drain stdin until EOF. The loop body is a deliberate no-op: the parent
/// closes stdin when it wants the child gone, and the `BufRead` structure
/// is the slot a future change can fill with a message dispatcher
/// without reshaping the binary.
fn drain_stdin_until_eof() -> ExitCode {
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

#[cfg(unix)]
fn install_sigterm_flag(flag: &Arc<AtomicBool>) -> std::io::Result<()> {
    use signal_hook::consts::SIGTERM;
    signal_hook::flag::register(SIGTERM, Arc::clone(flag))?;
    Ok(())
}

#[cfg(unix)]
fn install_sigterm_ignore() -> std::io::Result<()> {
    use signal_hook::consts::SIGTERM;
    // signal-hook's safe `flag::register` API replaces the default action
    // (terminate) with a "set this AtomicBool" handler. We register a flag
    // we never read, which is equivalent to ignoring SIGTERM from the
    // kernel's perspective: the signal arrives, the handler runs, the
    // handler returns, and the process keeps going. The bridge then has
    // to escalate to SIGKILL after its grace period.
    //
    // Going through `signal_hook::flag::register` (rather than the
    // closure-taking `signal_hook_registry::register` which is `unsafe`)
    // keeps this fixture compatible with the workspace's
    // `unsafe_code = "forbid"` lint.
    let sink = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, sink)?;
    Ok(())
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

fn emit_raw_stdout_line(line: &str) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}
