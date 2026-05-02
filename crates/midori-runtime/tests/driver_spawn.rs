//! Integration tests for the Bridge-side driver spawn / handshake flow.
//!
//! Each test runs `midori-driver-dummy` with one of three CLI flag shapes
//! (default / `--no-hello` / `--bad-version`) and asserts the matching
//! `spawn_driver` outcome. Cargo's `CARGO_BIN_EXE_<name>` env var only
//! covers binaries declared in the same package as the test, so the
//! fixture path is resolved at runtime by walking the workspace `target/`
//! tree the test executable itself lives in.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use midori_runtime::driver_proc::{spawn_driver_with_timeout, SpawnError, HANDSHAKE_TIMEOUT};

/// Resolved path to the `midori-driver-dummy` fixture binary.
///
/// Cargo's `CARGO_BIN_EXE_<name>` env var is only set for binaries in the
/// same package as the test, so this resolver walks the workspace `target/`
/// tree the test's own `current_exe()` lives under and locates the sibling
/// `midori-driver-dummy` artifact. The dummy crate is not a build-time
/// dependency of this test crate, so the resolver issues an explicit
/// `cargo build` when the artifact is missing rather than relying on
/// cargo's incidental ordering.
fn dummy_bin_path() -> &'static std::path::Path {
    static CELL: OnceLock<PathBuf> = OnceLock::new();
    CELL.get_or_init(|| {
        let test_exe = std::env::current_exe().expect("current_exe");
        // Integration test executables live at
        // `<workspace>/target/<profile>/deps/<test-name>-<hash>`; walk up
        // two levels to reach the profile dir.
        let profile_dir = test_exe
            .parent()
            .and_then(|p| p.parent())
            .expect("integration test exe must live under target/<profile>/deps/");
        let candidate = profile_dir.join(bin_filename("midori-driver-dummy"));
        if !candidate.exists() {
            // Force a build of the dummy binary into the same profile so
            // the test does not silently fall back to a stale or missing
            // artifact. Failure here is a hard test failure — there is no
            // graceful fallback that preserves the meaning of the suite.
            let status = std::process::Command::new(env!("CARGO"))
                .args(["build", "-p", "midori-driver-dummy"])
                .status()
                .expect("invoke cargo build for fixture");
            assert!(
                status.success(),
                "cargo build of midori-driver-dummy failed"
            );
        }
        assert!(
            candidate.exists(),
            "fixture binary not found at {}",
            candidate.display()
        );
        candidate
    })
}

#[cfg(windows)]
fn bin_filename(stem: &str) -> String {
    format!("{stem}.exe")
}

#[cfg(not(windows))]
fn bin_filename(stem: &str) -> String {
    stem.to_owned()
}

/// Table-driven layout: each row pins one fixture flag set to one expected
/// outcome class. The actual assertions live in the per-row helpers because
/// `SpawnError` is non-`Eq` and matching on its variants is the cleanest
/// way to assert without overspecifying inner fields.
struct SpawnCase {
    name: &'static str,
    fixture_args: &'static [&'static str],
    timeout: Duration,
}

const CASE_SUCCESS: SpawnCase = SpawnCase {
    name: "dummy-success",
    fixture_args: &[],
    // Production timeout is fine for the success path; the dummy emits hello
    // synchronously so the wait is effectively instantaneous.
    timeout: HANDSHAKE_TIMEOUT,
};

const CASE_TIMEOUT: SpawnCase = SpawnCase {
    name: "dummy-timeout",
    fixture_args: &["--no-hello"],
    // Short enough that the suite stays fast, long enough that a slow CI
    // runner does not race the dummy's stdin-read setup.
    timeout: Duration::from_millis(200),
};

const CASE_INCOMPAT: SpawnCase = SpawnCase {
    name: "dummy-incompat",
    fixture_args: &["--bad-version"],
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

#[cfg(unix)]
#[test]
fn it_should_return_handshake_timeout_when_driver_does_not_emit_hello() {
    let case = CASE_TIMEOUT;
    let err = spawn(&case).expect_err("--no-hello must surface as a timeout");
    assert!(
        matches!(err, SpawnError::HandshakeTimeout),
        "expected HandshakeTimeout, got {err:?}"
    );
}

#[cfg(unix)]
#[test]
fn it_should_return_incompatible_sdk_when_driver_advertises_unknown_version() {
    let case = CASE_INCOMPAT;
    let err = spawn(&case).expect_err("--bad-version must surface as IncompatibleSdk");
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

/// Build a `SpawnError`-returning call from a [`SpawnCase`]. The fixture
/// arguments are appended after the implicit `start` subcommand by the
/// dummy binary's own argv parser; `spawn_driver` itself always passes
/// `start`, and additional flags ride along via the binary path's args
/// embedded as a wrapper script.
///
/// Since `Command::new(path)` only takes one arg list and `spawn_driver`
/// already injects `start`, the helper materialises a small shell script
/// per call when extra flags are needed. The shell-script path is unix-only;
/// fixture-flag-using cases are gated with `#[cfg(unix)]` accordingly.
fn spawn(case: &SpawnCase) -> Result<midori_runtime::driver_proc::DriverHandle, SpawnError> {
    let bin_path = if case.fixture_args.is_empty() {
        dummy_bin_path().to_path_buf()
    } else {
        wrap_with_flags(case.name, case.fixture_args)
    };
    spawn_driver_with_timeout(case.name, bin_path, &serde_json::json!({}), case.timeout)
}

/// Materialise a tiny shell wrapper that invokes the dummy binary with the
/// supplied extra flags after the implicit `start` arg the Bridge always
/// injects. The wrapper lives under the system temp directory and is
/// kept around for the lifetime of the test process — small enough to
/// not warrant explicit cleanup.
#[cfg(unix)]
fn wrap_with_flags(tag: &str, extra_args: &[&str]) -> std::path::PathBuf {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;

    let dir = std::env::temp_dir().join(format!("midori-driver-spawn-test-{tag}"));
    std::fs::create_dir_all(&dir).expect("mkdir wrapper dir");
    let script_path = dir.join("dummy-wrapper.sh");
    let mut script = String::new();
    script.push_str("#!/bin/sh\n");
    script.push_str("exec ");
    let dummy_path = dummy_bin_path()
        .to_str()
        .expect("dummy bin path must be UTF-8");
    script.push_str(&shell_quote(dummy_path));
    script.push_str(" \"$@\"");
    for arg in extra_args {
        script.push(' ');
        script.push_str(&shell_quote(arg));
    }
    script.push('\n');
    let mut f = std::fs::File::create(&script_path).expect("create wrapper");
    f.write_all(script.as_bytes()).expect("write wrapper");
    drop(f);
    let mut perms = std::fs::metadata(&script_path)
        .expect("stat wrapper")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script_path, perms).expect("chmod wrapper");
    script_path
}

#[cfg(not(unix))]
fn wrap_with_flags(_tag: &str, _extra_args: &[&str]) -> std::path::PathBuf {
    panic!("driver_spawn integration tests currently require a unix shell")
}

#[cfg(unix)]
fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str(r"'\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}
