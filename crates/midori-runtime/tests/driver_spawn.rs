//! Integration tests for the Bridge-side driver spawn / handshake / lifecycle
//! flow.
//!
//! The fixture comes as multiple sibling binaries built from the same source
//! (`crates/midori-driver-dummy/`). Each binary's behavior is selected at
//! build time via `CARGO_BIN_NAME`, so the test hands `spawn_driver` an
//! exact path per case and never has to materialise a wrapper script
//! at runtime — wrappers race against parallel tests under Linux fork-exec
//! semantics (ETXTBSY).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use midori_runtime::driver_proc::{
    spawn_driver_with_options, DriverHandle, SpawnError, SpawnOptions, HANDSHAKE_TIMEOUT,
};

/// Resolved path to a fixture binary by name.
///
/// Cargo's `CARGO_BIN_EXE_<name>` env var is only set for binaries declared
/// in the same package as the test, so the resolver walks the workspace
/// `target/<profile>/` directory the test executable itself lives under.
/// If the artifact is missing (e.g. `cargo test -p midori-runtime` without
/// `--workspace`) the resolver builds it explicitly. Each binary is cached
/// in its own `OnceLock` so the build is amortised across tests.
fn fixture_bin_path(bin_name: &'static str) -> &'static std::path::Path {
    fn resolve(bin_name: &str) -> PathBuf {
        let test_exe = std::env::current_exe().expect("current_exe");
        let profile_dir = test_exe
            .parent()
            .and_then(|p| p.parent())
            .expect("integration test exe must live under target/<profile>/deps/");
        // The directory name immediately above `deps/` is the cargo profile
        // the test was compiled under: `target/<profile>/deps/<test-bin>`.
        // We pass it back to `cargo build` so the fixture lands in the same
        // profile dir the resolver is searching in. `--profile dev` is the
        // canonical spelling for the directory that cargo names `debug`.
        let profile_name = profile_dir
            .file_name()
            .and_then(|n| n.to_str())
            .expect("profile directory name must be UTF-8");
        let candidate = profile_dir.join(bin_filename(bin_name));
        if !candidate.exists() {
            let cargo_profile = if profile_name == "debug" {
                "dev"
            } else {
                profile_name
            };
            let status = std::process::Command::new(env!("CARGO"))
                .args([
                    "build",
                    "-p",
                    "midori-driver-dummy",
                    "--bin",
                    bin_name,
                    "--profile",
                    cargo_profile,
                ])
                .status()
                .expect("invoke cargo build for fixture");
            assert!(
                status.success(),
                "cargo build of {bin_name} (profile {cargo_profile}) failed"
            );
        }
        assert!(
            candidate.exists(),
            "fixture binary not found at {}",
            candidate.display()
        );
        candidate
    }

    macro_rules! cached {
        ($name:literal) => {{
            static CELL: OnceLock<PathBuf> = OnceLock::new();
            CELL.get_or_init(|| resolve($name))
        }};
    }

    match bin_name {
        "midori-driver-dummy" => cached!("midori-driver-dummy"),
        "midori-driver-dummy-no-hello" => cached!("midori-driver-dummy-no-hello"),
        "midori-driver-dummy-bad-version" => cached!("midori-driver-dummy-bad-version"),
        "midori-driver-dummy-noisy-stdout" => cached!("midori-driver-dummy-noisy-stdout"),
        "midori-driver-dummy-sigterm-graceful" => cached!("midori-driver-dummy-sigterm-graceful"),
        "midori-driver-dummy-sigterm-ignore" => cached!("midori-driver-dummy-sigterm-ignore"),
        "midori-driver-dummy-exit-error" => cached!("midori-driver-dummy-exit-error"),
        other => panic!("unknown fixture binary requested: {other}"),
    }
}

#[cfg(windows)]
fn bin_filename(stem: &str) -> String {
    format!("{stem}.exe")
}

#[cfg(not(windows))]
fn bin_filename(stem: &str) -> String {
    stem.to_owned()
}

/// Table-driven layout: each row pins one fixture binary to one expected
/// handshake outcome class. The actual assertions live in the per-row
/// helpers because `SpawnError` is non-`Eq` and matching on its variants
/// is the cleanest way to assert without overspecifying inner fields.
struct SpawnCase {
    name: &'static str,
    fixture_bin: &'static str,
    options: SpawnOptions,
}

const fn options_with_timeout(timeout: Duration) -> SpawnOptions {
    SpawnOptions {
        handshake_timeout: timeout,
        // Tests that exercise the handshake path do not hold the resulting
        // handle long, so the shutdown grace value is irrelevant — we use
        // the production default for parity with `spawn_driver`.
        shutdown_grace: Duration::from_secs(3),
    }
}

const CASE_SUCCESS: SpawnCase = SpawnCase {
    name: "dummy-success",
    fixture_bin: "midori-driver-dummy",
    // Production timeout is fine for the success path; the dummy emits hello
    // synchronously so the wait is effectively instantaneous.
    options: options_with_timeout(HANDSHAKE_TIMEOUT),
};

const CASE_TIMEOUT: SpawnCase = SpawnCase {
    name: "dummy-timeout",
    fixture_bin: "midori-driver-dummy-no-hello",
    // Short enough that the suite stays fast, long enough that a slow CI
    // runner does not race the dummy's stdin-read setup.
    options: options_with_timeout(Duration::from_millis(200)),
};

const CASE_INCOMPAT: SpawnCase = SpawnCase {
    name: "dummy-incompat",
    fixture_bin: "midori-driver-dummy-bad-version",
    options: options_with_timeout(HANDSHAKE_TIMEOUT),
};

#[test]
fn it_should_complete_handshake_against_dummy_driver() {
    let case = CASE_SUCCESS;
    let handle = spawn(&case).unwrap_or_else(|err| {
        panic!("expected success, got {err:?}");
    });
    assert_eq!(handle.name, case.name);
    assert!(
        !handle.sdk_version.is_empty(),
        "driver must report a non-empty SDK version on the success path"
    );
    // Dropping the handle drops `ChildStdin` first, which closes the write
    // half of the pipe. The dummy's BufRead loop terminates on the resulting
    // EOF, so the SIGTERM escalation in `Drop` finds the child already gone.
    drop(handle);
}

#[test]
fn it_should_return_handshake_timeout_when_driver_does_not_emit_hello() {
    let case = CASE_TIMEOUT;
    let err = spawn(&case).expect_err("no-hello fixture must surface as a timeout");
    match err {
        SpawnError::HandshakeTimeout(elapsed) => {
            assert_eq!(
                elapsed, case.options.handshake_timeout,
                "HandshakeTimeout must echo the caller-supplied timeout, got {elapsed:?}"
            );
        }
        other => panic!("expected HandshakeTimeout, got {other:?}"),
    }
}

#[test]
fn it_should_return_incompatible_sdk_when_driver_advertises_unknown_version() {
    let case = CASE_INCOMPAT;
    let err = spawn(&case).expect_err("bad-version fixture must surface as IncompatibleSdk");
    match err {
        SpawnError::IncompatibleSdk {
            driver_version,
            reason,
        } => {
            assert!(
                driver_version.starts_with("99."),
                "driver_version should echo the advertised value, got {driver_version}"
            );
            assert!(
                reason.contains("major"),
                "reason should describe the major-mismatch, got {reason}"
            );
        }
        other => panic!("expected IncompatibleSdk, got {other:?}"),
    }
}

// ------------------------------------------------------------------
// Lifecycle tests: control messages, log forwarding, abnormal exit,
// SIGTERM → grace → SIGKILL escalation.
// ------------------------------------------------------------------

/// Shorter shutdown grace used by lifecycle tests so the SIGKILL-escalation
/// case finishes inside a reasonable test budget. The value is large enough
/// that the SIGTERM-graceful fixture's poll loop (20ms) runs at least a
/// handful of times under load before the deadline fires.
const TEST_SHUTDOWN_GRACE: Duration = Duration::from_millis(300);

fn lifecycle_options() -> SpawnOptions {
    SpawnOptions {
        handshake_timeout: HANDSHAKE_TIMEOUT,
        shutdown_grace: TEST_SHUTDOWN_GRACE,
    }
}

#[test]
fn it_should_complete_full_lifecycle_against_sigterm_graceful_dummy() {
    // spawn → connect → disconnect → configure → drop. The
    // SIGTERM-graceful fixture flips its shutdown flag on SIGTERM so Drop's
    // grace period is enough to observe a clean exit before the SIGKILL
    // escalation triggers.
    let bin = fixture_bin_path("midori-driver-dummy-sigterm-graceful").to_path_buf();
    let mut handle =
        spawn_driver_with_options("dummy", bin, &serde_json::json!({}), &lifecycle_options())
            .expect("handshake against sigterm-graceful fixture");

    handle
        .connect("port-a", &serde_json::json!({"latency_ms": 1}))
        .expect("connect write succeeds while child stdin is open");
    handle
        .configure(&serde_json::json!({"sample_rate": 48_000}))
        .expect("configure write succeeds");
    handle
        .disconnect("port-a")
        .expect("disconnect write succeeds");
    assert!(
        handle.is_alive(),
        "child should still be alive between control writes"
    );

    let started = Instant::now();
    drop(handle);
    let elapsed = started.elapsed();
    assert!(
        elapsed < TEST_SHUTDOWN_GRACE * 4,
        "graceful drop should finish well within escalation budget, took {elapsed:?}"
    );
}

#[test]
fn it_should_record_abnormal_exit_when_driver_exits_with_non_zero_status() {
    let bin = fixture_bin_path("midori-driver-dummy-exit-error").to_path_buf();
    let handle = spawn_driver_with_options(
        "dummy-exit",
        bin,
        &serde_json::json!({}),
        &lifecycle_options(),
    )
    .expect("handshake completes before the child exits");

    // Watcher thread races the child's exit. Poll `is_alive` for up to a
    // second; this is a generous bound because spawn → exit is microseconds
    // on the dummy.
    let deadline = Instant::now() + Duration::from_secs(1);
    while handle.is_alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !handle.is_alive(),
        "watcher should mark the handle dead after the child's non-zero exit"
    );
    drop(handle);
}

#[test]
fn it_should_escalate_to_sigkill_when_sigterm_is_ignored() {
    // The fixture installs a SIGTERM handler that does nothing, and ignores
    // stdin EOF. The bridge's Drop must escalate to SIGKILL after the grace
    // period elapses. Wall-clock budget: at most `grace + a generous slack`
    // — we measure the elapsed time across `drop(handle)` and assert it
    // stays below ~1 second on a 300ms grace.
    let bin = fixture_bin_path("midori-driver-dummy-sigterm-ignore").to_path_buf();
    let handle = spawn_driver_with_options(
        "dummy-ignore",
        bin,
        &serde_json::json!({}),
        &lifecycle_options(),
    )
    .expect("handshake against sigterm-ignore fixture");

    let started = Instant::now();
    drop(handle);
    let elapsed = started.elapsed();
    // Lower bound: the grace period must have actually elapsed because the
    // fixture cannot exit any earlier than that.
    assert!(
        elapsed >= TEST_SHUTDOWN_GRACE,
        "SIGKILL escalation should not fire before grace period, took {elapsed:?}"
    );
    // Upper bound: the SIGKILL path completes shortly after the grace
    // deadline. Wide budget to absorb scheduler jitter on busy CI runners.
    assert!(
        elapsed < TEST_SHUTDOWN_GRACE + Duration::from_secs(2),
        "SIGKILL escalation should finish soon after grace period, took {elapsed:?}"
    );
}

#[test]
fn it_should_spawn_noisy_stdout_fixture_without_hanging() {
    // The bridge-internal log-forwarder thread is wired to crate::logging,
    // which writes to the real `io::stdout()`. Capturing that without
    // refactoring the logging sink is out of scope, so this is a smoke
    // test: the fixture emits a non-JSON line, the forwarder consumes it,
    // and Drop completes. A regression that re-uses the handshake reader
    // (or fails to start the forwarder) would manifest as a hang here.
    let bin = fixture_bin_path("midori-driver-dummy-noisy-stdout").to_path_buf();
    let handle =
        spawn_driver_with_options("noisy", bin, &serde_json::json!({}), &lifecycle_options())
            .expect("handshake against noisy-stdout fixture");
    // Give the forwarder thread a beat to consume the emitted line. This is
    // not load-bearing for correctness but makes the test's intent explicit.
    std::thread::sleep(Duration::from_millis(50));
    drop(handle);
}

/// Build a `SpawnError`-returning call from a [`SpawnCase`]. Each case
/// resolves to a pre-built binary; no wrapper script or runtime fixture
/// argument injection is needed.
fn spawn(case: &SpawnCase) -> Result<DriverHandle, SpawnError> {
    let bin_path = fixture_bin_path(case.fixture_bin).to_path_buf();
    spawn_driver_with_options(case.name, bin_path, &serde_json::json!({}), &case.options)
}
