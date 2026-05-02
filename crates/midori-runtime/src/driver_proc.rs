//! Driver subprocess supervisor: spawn a driver binary, complete the JSON
//! Lines `hello` / `hello_ack` handshake, and hand back a [`DriverHandle`].
//!
//! # Protocol
//!
//! The wire format is `design/10-driver-plugin.md`「バージョン互換性」:
//!
//! ```jsonc
//! // Driver → Bridge stdout (first line)
//! {"type": "hello", "sdk_version": "1.0.0"}
//!
//! // Bridge → Driver stdin (first line)
//! {"type": "hello_ack", "compatible": true}
//! // or
//! {"type": "hello_ack", "compatible": false, "reason": "sdk 1.0.0 is too old, require >=1.2.0"}
//! ```
//!
//! # Concurrency model
//!
//! The handshake is implemented with synchronous primitives — `std::process`
//! for the child, a worker thread that reads the first stdout line into a
//! `std::sync::mpsc` channel, and `Receiver::recv_timeout` for the bound on
//! handshake duration. The runtime crate does not depend on tokio, and the
//! handshake is short-lived enough that a thread-per-spawn model is fine.
//!
//! # Scope
//!
//! Only handshake completion is implemented here. Lifecycle management
//! (graceful shutdown, log forwarding from the post-handshake stdout stream,
//! SIGTERM / SIGKILL escalation) is the responsibility of the next subtask
//! and intentionally absent from this module.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Default upper bound on how long [`spawn_driver`] waits for the driver to
/// emit its `hello` line. Drivers are expected to write `hello` synchronously
/// at the top of `main()`, so 5 seconds is multiple orders of magnitude
/// beyond the realistic budget. Tests use [`spawn_driver_with_timeout`] with
/// a much shorter value to keep the suite fast.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// SDK majors the Bridge currently understands. The compatibility check
/// accepts any `hello.sdk_version` parseable as `MAJOR.MINOR.PATCH` whose
/// `MAJOR` is listed here. Bumping the SDK's binary layout extends this list
/// (or replaces it) — see `design/10-driver-plugin.md`「バージョン互換性」 for
/// the policy: layout changes happen on semver-major bumps only.
const ACCEPTED_SDK_MAJORS: &[u32] = &[0, 1];

/// JSON-tagged `hello` message read from the driver's stdout.
#[derive(Debug, serde::Deserialize)]
struct HelloMessage {
    #[serde(rename = "type")]
    message_type: String,
    sdk_version: String,
}

/// JSON-tagged `hello_ack` message written to the driver's stdin.
#[derive(Debug, serde::Serialize)]
struct HelloAckMessage<'a> {
    #[serde(rename = "type")]
    message_type: &'a str,
    compatible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
}

/// Outcome of a `spawn_driver` call.
///
/// On the success path the handle owns the child process plus its stdin
/// writer; the post-handshake stdout reader thread is intentionally not
/// exposed yet (subtask (b) will wire log forwarding through it). Dropping
/// the handle closes stdin, which the dummy fixture treats as the signal to
/// exit its `BufRead` loop.
#[derive(Debug)]
pub struct DriverHandle {
    /// Driver name as supplied to [`spawn_driver`]. Carried for diagnostics.
    pub name: String,
    /// Resolved binary path, useful for log lines and crash reports.
    pub path: PathBuf,
    /// Profile blob forwarded to the driver. Held for later use by subtask
    /// (b) when `connect` / `configure` flows are wired in.
    pub profile: serde_json::Value,
    /// SDK version the driver advertised in its `hello`.
    pub sdk_version: String,
    child: Child,
    stdin: ChildStdin,
}

impl DriverHandle {
    /// Borrow the child process handle (for `id()` and similar inspection).
    #[must_use]
    pub fn child(&self) -> &Child {
        &self.child
    }

    /// Mutable borrow of the child stdin pipe so subtask (b) can write
    /// follow-up control commands without taking the pipe.
    pub fn stdin_mut(&mut self) -> &mut ChildStdin {
        &mut self.stdin
    }
}

/// Errors returned by [`spawn_driver`] / [`spawn_driver_with_timeout`].
#[derive(Debug)]
pub enum SpawnError {
    /// `Command::spawn` itself failed (binary missing, permission denied, …).
    Spawn(std::io::Error),
    /// The driver process started but did not emit `hello` within the
    /// configured timeout. The child is killed before this is returned.
    HandshakeTimeout,
    /// The driver closed stdout (and/or exited) without ever emitting
    /// `hello`. This is the EOF-before-handshake path.
    HandshakeMissing,
    /// A line was received but failed to parse as a `hello` message.
    HandshakeMalformed(serde_json::Error),
    /// The driver advertised an SDK version the Bridge does not accept.
    /// Before this is returned, the Bridge writes `hello_ack { compatible:
    /// false }` so the driver can shut down cleanly with a logged reason.
    IncompatibleSdk {
        driver_version: String,
        reason: String,
    },
    /// Writing the `hello_ack` line to the driver's stdin failed.
    HelloAckWrite(std::io::Error),
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(err) => write!(f, "driver プロセスの spawn に失敗しました: {err}"),
            Self::HandshakeTimeout => write!(
                f,
                "driver が {} 秒以内に hello を送信しませんでした",
                HANDSHAKE_TIMEOUT.as_secs()
            ),
            Self::HandshakeMissing => {
                f.write_str("driver が hello を送信せずに stdout を閉じました")
            }
            Self::HandshakeMalformed(err) => {
                write!(
                    f,
                    "driver の hello を JSON として解釈できませんでした: {err}"
                )
            }
            Self::IncompatibleSdk {
                driver_version,
                reason,
            } => write!(
                f,
                "driver の SDK バージョン `{driver_version}` は Bridge と非互換です: {reason}"
            ),
            Self::HelloAckWrite(err) => write!(
                f,
                "driver stdin への hello_ack 書き込みに失敗しました: {err}"
            ),
        }
    }
}

impl std::error::Error for SpawnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(err) | Self::HelloAckWrite(err) => Some(err),
            Self::HandshakeMalformed(err) => Some(err),
            Self::HandshakeTimeout | Self::HandshakeMissing | Self::IncompatibleSdk { .. } => None,
        }
    }
}

/// Spawn a driver binary at `path`, complete the version handshake, and
/// return a [`DriverHandle`] tied to the live child.
///
/// `name` is the driver identifier (used for diagnostics and later for
/// dispatching events); `profile` is the configuration blob the runtime
/// will hand to the driver after the handshake (subtask (b) uses it).
///
/// On the failure path the child process is reaped before this function
/// returns — callers do not need to clean up zombies on `Err(_)`.
pub fn spawn_driver(
    name: impl Into<String>,
    path: impl Into<PathBuf>,
    profile: serde_json::Value,
) -> Result<DriverHandle, SpawnError> {
    spawn_driver_with_timeout(name, path, profile, HANDSHAKE_TIMEOUT)
}

/// [`spawn_driver`] with an explicit handshake timeout. Tests use a short
/// timeout (typically 200ms) to keep the suite fast; production callers
/// should keep using the default via [`spawn_driver`].
pub fn spawn_driver_with_timeout(
    name: impl Into<String>,
    path: impl Into<PathBuf>,
    profile: serde_json::Value,
    handshake_timeout: Duration,
) -> Result<DriverHandle, SpawnError> {
    let name = name.into();
    let path = path.into();

    let mut child = Command::new(&path)
        .arg("start")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(SpawnError::Spawn)?;

    // Take pipes immediately so we can hand them to threads without holding
    // an outstanding `&mut child` borrow during the handshake.
    let stdout = child
        .stdout
        .take()
        .expect("Stdio::piped() guarantees stdout is Some");
    let mut stdin = child
        .stdin
        .take()
        .expect("Stdio::piped() guarantees stdin is Some");

    // Read the first stdout line on a worker thread. Channel-disconnect
    // (sender dropped without sending) maps to EOF before hello.
    let (tx, rx) = mpsc::channel::<std::io::Result<String>>();
    let reader_handle = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // Sender drops on return; the Bridge sees Disconnected.
            }
            Ok(_) => {
                let _ = tx.send(Ok(line));
            }
            Err(err) => {
                let _ = tx.send(Err(err));
            }
        }
        // Hand the BufReader back via thread join so subtask (b) can resume
        // reading subsequent stdout lines (log forwarding) once the
        // join-API for that path is designed. For now the reader is
        // dropped, which closes the read half cleanly.
        drop(reader);
    });

    let hello_line = match rx.recv_timeout(handshake_timeout) {
        Ok(Ok(line)) => line,
        Ok(Err(io_err)) => {
            kill_and_reap(&mut child);
            // Reap the worker thread so its drop runs on this thread
            // rather than detaching.
            let _ = reader_handle.join();
            return Err(SpawnError::Spawn(io_err));
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            kill_and_reap(&mut child);
            let _ = reader_handle.join();
            return Err(SpawnError::HandshakeTimeout);
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            kill_and_reap(&mut child);
            let _ = reader_handle.join();
            return Err(SpawnError::HandshakeMissing);
        }
    };

    let _ = reader_handle.join();

    let hello: HelloMessage = match serde_json::from_str(hello_line.trim_end()) {
        Ok(parsed) => parsed,
        Err(err) => {
            kill_and_reap(&mut child);
            return Err(SpawnError::HandshakeMalformed(err));
        }
    };

    if hello.message_type != "hello" {
        kill_and_reap(&mut child);
        return Err(SpawnError::HandshakeMalformed(json_type_error(&format!(
            "expected `type=\"hello\"`, got `type={:?}`",
            hello.message_type
        ))));
    }

    let compat = is_sdk_compatible(&hello.sdk_version);
    let ack = HelloAckMessage {
        message_type: "hello_ack",
        compatible: compat.is_compatible(),
        reason: compat.reason(),
    };

    if let Err(err) = write_hello_ack(&mut stdin, &ack) {
        kill_and_reap(&mut child);
        return Err(SpawnError::HelloAckWrite(err));
    }

    if let Compatibility::Incompatible { reason } = compat {
        // Give the driver a brief window to observe the negative ack and
        // exit cleanly. If it doesn't, escalate to kill so the parent does
        // not block. Subtask (b) replaces this with the SIGTERM/SIGKILL
        // ladder agreed for the lifecycle module.
        wait_for_exit_or_kill(&mut child, Duration::from_millis(200));
        return Err(SpawnError::IncompatibleSdk {
            driver_version: hello.sdk_version,
            reason,
        });
    }

    Ok(DriverHandle {
        name,
        path,
        profile,
        sdk_version: hello.sdk_version,
        child,
        stdin,
    })
}

/// Result of [`is_sdk_compatible`]. Carried as a small enum so the caller
/// can build the `hello_ack` reason field and the `IncompatibleSdk` error
/// from the same source of truth.
enum Compatibility {
    Compatible,
    Incompatible { reason: String },
}

impl Compatibility {
    fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible)
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Compatible => None,
            Self::Incompatible { reason } => Some(reason.as_str()),
        }
    }
}

/// Decide whether the Bridge accepts `version` as a peer SDK.
///
/// The accepted set is `MAJOR.MINOR.PATCH` triples whose `MAJOR` is listed
/// in [`ACCEPTED_SDK_MAJORS`]. Anything that fails to parse — empty string,
/// non-numeric segments, fewer than three segments — is rejected with a
/// reason describing the failure mode so the driver-side log makes the
/// mismatch obvious.
fn is_sdk_compatible(version: &str) -> Compatibility {
    let mut parts = version.split('.');
    let Some(major_str) = parts.next() else {
        return Compatibility::Incompatible {
            reason: format!("sdk_version `{version}` is empty"),
        };
    };
    let Ok(major) = major_str.parse::<u32>() else {
        return Compatibility::Incompatible {
            reason: format!("sdk_version `{version}` has non-numeric major component"),
        };
    };
    // Require the remaining `MINOR.PATCH` segments to exist, even if we don't
    // gate on their values — the protocol contract is "MAJOR.MINOR.PATCH".
    if parts.next().is_none() || parts.next().is_none() {
        return Compatibility::Incompatible {
            reason: format!("sdk_version `{version}` is not in MAJOR.MINOR.PATCH form"),
        };
    }
    if !ACCEPTED_SDK_MAJORS.contains(&major) {
        return Compatibility::Incompatible {
            reason: format!(
                "sdk_version `{version}` advertises major {major}, Bridge accepts {ACCEPTED_SDK_MAJORS:?}"
            ),
        };
    }
    Compatibility::Compatible
}

/// Encode and send a `hello_ack` line on `stdin`.
fn write_hello_ack(stdin: &mut ChildStdin, ack: &HelloAckMessage<'_>) -> std::io::Result<()> {
    serde_json::to_writer(&mut *stdin, ack)?;
    stdin.write_all(b"\n")?;
    stdin.flush()
}

/// Kill the child and wait for it so callers do not leak zombies on the
/// error paths. Errors during kill / wait are intentionally swallowed: the
/// outer `Err(SpawnError::…)` already carries the user-visible cause and
/// the cleanup is best-effort.
fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Poll for the child to exit; if it still hasn't after `grace`, kill it.
/// Used on the `IncompatibleSdk` path so the driver gets a chance to log
/// the negative ack and exit on its own before the Bridge force-closes it.
fn wait_for_exit_or_kill(child: &mut Child, grace: Duration) {
    let deadline = std::time::Instant::now() + grace;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    kill_and_reap(child);
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => {
                kill_and_reap(child);
                return;
            }
        }
    }
}

/// Synthesize a `serde_json::Error` for the "valid JSON but wrong tag" case
/// so [`SpawnError::HandshakeMalformed`] can carry the diagnostic without a
/// dedicated variant.
fn json_type_error(message: &str) -> serde_json::Error {
    // serde_json does not expose `Error::custom` publicly; round-trip
    // through `serde::de::Error` via a deserializer that produces our text.
    use serde::de::Error as _;
    serde_json::Error::custom(message)
}
