//! Integration tests for the Bridge-side driver spawn / handshake flow.
//!
//! The fixture comes as three sibling binaries built from the same source
//! (`crates/midori-driver-dummy/`). Each binary's behavior is selected at
//! build time via `CARGO_BIN_NAME`, so the test hands `spawn_driver` an
//! exact path per case and never has to materialise a wrapper script
//! at runtime — wrappers race against parallel tests under Linux fork-exec
//! semantics (ETXTBSY).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use midori_runtime::driver_proc::{spawn_driver_with_timeout, SpawnError, HANDSHAKE_TIMEOUT};

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

    match bin_name {
        "midori-driver-dummy" => {
            static CELL: OnceLock<PathBuf> = OnceLock::new();
            CELL.get_or_init(|| resolve(bin_name))
        }
        "midori-driver-dummy-no-hello" => {
            static CELL: OnceLock<PathBuf> = OnceLock::new();
            CELL.get_or_init(|| resolve(bin_name))
        }
        "midori-driver-dummy-bad-version" => {
            static CELL: OnceLock<PathBuf> = OnceLock::new();
            CELL.get_or_init(|| resolve(bin_name))
        }
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
/// outcome class. The actual assertions live in the per-row helpers because
/// `SpawnError` is non-`Eq` and matching on its variants is the cleanest
/// way to assert without overspecifying inner fields.
struct SpawnCase {
    name: &'static str,
    fixture_bin: &'static str,
    timeout: Duration,
}

const CASE_SUCCESS: SpawnCase = SpawnCase {
    name: "dummy-success",
    fixture_bin: "midori-driver-dummy",
    // Production timeout is fine for the success path; the dummy emits hello
    // synchronously so the wait is effectively instantaneous.
    timeout: HANDSHAKE_TIMEOUT,
};

const CASE_TIMEOUT: SpawnCase = SpawnCase {
    name: "dummy-timeout",
    fixture_bin: "midori-driver-dummy-no-hello",
    // Short enough that the suite stays fast, long enough that a slow CI
    // runner does not race the dummy's stdin-read setup.
    timeout: Duration::from_millis(200),
};

const CASE_INCOMPAT: SpawnCase = SpawnCase {
    name: "dummy-incompat",
    fixture_bin: "midori-driver-dummy-bad-version",
    timeout: HANDSHAKE_TIMEOUT,
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
    // EOF. The `Child` itself is not awaited here; lifecycle handling lives
    // outside this module.
    drop(handle);
}

#[test]
fn it_should_return_handshake_timeout_when_driver_does_not_emit_hello() {
    let case = CASE_TIMEOUT;
    let err = spawn(&case).expect_err("no-hello fixture must surface as a timeout");
    match err {
        SpawnError::HandshakeTimeout(elapsed) => {
            assert_eq!(
                elapsed, case.timeout,
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

/// Build a `SpawnError`-returning call from a [`SpawnCase`]. Each case
/// resolves to a pre-built binary; no wrapper script or runtime fixture
/// argument injection is needed.
fn spawn(case: &SpawnCase) -> Result<midori_runtime::driver_proc::DriverHandle, SpawnError> {
    let bin_path = fixture_bin_path(case.fixture_bin).to_path_buf();
    spawn_driver_with_timeout(case.name, bin_path, &serde_json::json!({}), case.timeout)
}
