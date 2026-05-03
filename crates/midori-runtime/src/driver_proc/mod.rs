//! Driver subprocess supervisor: spawn a driver binary, complete the JSON
//! Lines `hello` / `hello_ack` handshake, and hand back a [`DriverHandle`].
//!
//! # Protocol
//!
//! The wire format is `design/10-driver-plugin.md`「バージョン互換性」「通信
//! アーキテクチャ」:
//!
//! ```jsonc
//! // Driver → Bridge stdout (first line)
//! {"type": "hello", "sdk_version": "1.0.0"}
//!
//! // Bridge → Driver stdin (first line)
//! {"type": "hello_ack", "compatible": true}
//! // or
//! {"type": "hello_ack", "compatible": false, "reason": "sdk 1.0.0 is too old, require >=1.2.0"}
//!
//! // Bridge → Driver stdin (post-handshake control messages)
//! {"type": "connect",    "device": "...", "config": {...}}
//! {"type": "disconnect", "device": "..."}
//! {"type": "configure",  "payload": {...}}
//! ```
//!
//! Control messages are fire-and-forget: the bridge does not wait on a
//! response. Driver-originated stdout lines after `hello` are JSON Lines
//! containing events / state, and any line that fails to parse as JSON is
//! treated as a debug log line and forwarded to the bridge logger under the
//! `driver/<name>` layer (spec line 150: "stdout の行が有効な JSON でなけれ
//! ばデバッグログとしてイベントログに転送する").
//!
//! # Concurrency model
//!
//! The handshake is implemented with synchronous primitives — `std::process`
//! for the child, a worker thread that reads the first stdout line into a
//! `std::sync::mpsc` channel, and `Receiver::recv_timeout` for the bound on
//! handshake duration. The runtime crate does not depend on tokio, and the
//! handshake is short-lived enough that a thread-per-spawn model is fine.
//!
//! After the handshake, ownership of the stdout `BufReader` transfers into a
//! long-lived "log forwarder" thread that drains the rest of the stream into
//! [`crate::logging`]. A second long-lived "watcher" thread calls
//! `child.wait()` to detect abnormal exits, flipping a shared
//! `Arc<AtomicBool>` that backs [`DriverHandle::is_alive`].
//!
//! # Lifecycle / shutdown
//!
//! [`DriverHandle`] owns the child process. Its [`Drop`] impl performs the
//! "SIGTERM → grace period → SIGKILL" escalation required by
//! `design/10-driver-plugin.md`「Bridge によるライフサイクル管理」:
//!
//! 1. Send `SIGTERM` (Unix only — Windows lifecycle is out of scope).
//! 2. Poll `child.try_wait()` until the grace period elapses.
//! 3. If the child has not exited, escalate to `child.kill()` (SIGKILL on
//!    Unix) and `wait()` the corpse so the caller does not leak a zombie.
//!
//! Tests construct handles via [`spawn_driver_with_options`] with a short
//! grace period to keep the suite fast.

use std::io::Write;
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

mod handshake;
mod lifecycle;
mod log_forward;
mod request_ring;

pub use log_forward::{non_json_line_to_log_event, LogEvent};
pub use request_ring::{
    try_parse_request_ring, RequestRingMessage, RingReadyMessage, RingRejectedMessage,
    REQUEST_RING_TYPE, RING_READY_TYPE, RING_REJECTED_TYPE,
};

use handshake::{
    is_sdk_compatible, json_type_error, read_hello_line, write_hello_ack, Compatibility,
    HelloAckMessage, HelloMessage,
};
use lifecycle::{kill_and_reap, spawn_exit_watcher, terminate_with_grace};
use log_forward::spawn_log_forwarder;

/// Default upper bound on how long [`spawn_driver`] waits for the driver to
/// emit its `hello` line. Drivers are expected to write `hello` synchronously
/// at the top of `main()`, so 5 seconds is multiple orders of magnitude
/// beyond the realistic budget. Tests use [`spawn_driver_with_options`] with
/// a much shorter value to keep the suite fast.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Default grace period between SIGTERM and SIGKILL when [`DriverHandle`] is
/// dropped. The driver SDK installs a SIGTERM handler that flips a shutdown
/// flag and exits its main loop on the next iteration; 3 seconds is well
/// above the realistic exit budget while still putting an outer bound on
/// how long a misbehaving driver can stall bridge shutdown.
pub const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// Tunable options for [`spawn_driver_with_options`]. Held as a struct so
/// adding more fields (e.g. environment, working directory) does not blow
/// up the function signature — see `limit-function-arguments`.
#[derive(Debug, Clone)]
pub struct SpawnOptions {
    /// Maximum time to wait for the driver's `hello` line before giving up.
    pub handshake_timeout: Duration,
    /// Time the [`DriverHandle::drop`] escalation waits between SIGTERM and
    /// SIGKILL.
    pub shutdown_grace: Duration,
}

impl Default for SpawnOptions {
    fn default() -> Self {
        Self {
            handshake_timeout: HANDSHAKE_TIMEOUT,
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
        }
    }
}

/// Outcome of a `spawn_driver` call.
///
/// On the success path the handle owns the driver's stdin writer plus
/// background threads that:
/// - own the child process and `wait()` for it on a dedicated thread
///   (the "watcher"), flipping a shared liveness flag when the child exits;
/// - drain the post-handshake stdout stream and forward each line
///   into the bridge logger (the "log forwarder").
///
/// Dropping the handle closes stdin, sends SIGTERM, polls the liveness
/// flag for the configured grace period, and finally escalates to SIGKILL
/// before joining both background threads.
#[derive(Debug)]
pub struct DriverHandle {
    /// Driver name as supplied to [`spawn_driver`]. Carried for diagnostics
    /// and used as the `driver/<name>` layer string for forwarded log lines.
    pub name: String,
    /// Resolved binary path, useful for log lines and crash reports.
    pub path: PathBuf,
    /// SDK version the driver advertised in its `hello`.
    pub sdk_version: String,
    /// PID captured at spawn time, used by [`Drop`] to send SIGTERM /
    /// SIGKILL via `nix` rather than going through `Child::kill` (which
    /// would require shared mutable access to the `Child` the watcher
    /// thread already owns).
    child_pid: u32,
    stdin: Option<ChildStdin>,
    /// Shared liveness flag. The watcher flips this to `false` when the
    /// child exits (graceful or abnormal). Exposed via [`Self::is_alive`]
    /// so callers can detect "this driver is gone, drop my handle", and
    /// used by [`Drop`] to detect a clean exit during the grace window.
    alive: Arc<AtomicBool>,
    /// Set by [`Drop`] before it sends SIGTERM, so the watcher thread can
    /// distinguish "we asked for this exit" (normal shutdown — do not
    /// surface as an error) from "the driver crashed on its own" (abnormal
    /// — log to bridge error).
    shutdown_initiated: Arc<AtomicBool>,
    /// Time to wait between SIGTERM and SIGKILL during [`Drop`].
    shutdown_grace: Duration,
    /// Background threads spawned at handshake completion. Joined on drop.
    log_forwarder: Option<JoinHandle<()>>,
    watcher: Option<JoinHandle<()>>,
}

impl DriverHandle {
    /// Borrow the driver name. Useful as the `driver/<name>` layer string.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the driver subprocess is still believed to be alive.
    ///
    /// The watcher thread sets this to `false` when `child.wait()` returns,
    /// so a `false` reading is authoritative ("the kernel reaped the child").
    /// A `true` reading is best-effort — the child could exit between the
    /// load and the next call.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Send a `connect` control message to the driver.
    ///
    /// `device` identifies the connection target (e.g. a MIDI port name);
    /// `config` is the driver-defined connection blob. Both are forwarded
    /// verbatim — the bridge does not interpret `config`. No response is
    /// awaited; control messages are fire-and-forget per the spec.
    pub fn connect(&mut self, device: &str, config: &serde_json::Value) -> std::io::Result<()> {
        self.write_control(&serde_json::json!({
            "type": "connect",
            "device": device,
            "config": config,
        }))
    }

    /// Send a `disconnect` control message for `device`.
    pub fn disconnect(&mut self, device: &str) -> std::io::Result<()> {
        self.write_control(&serde_json::json!({
            "type": "disconnect",
            "device": device,
        }))
    }

    /// Send a `configure` control message carrying an opaque payload.
    pub fn configure(&mut self, payload: &serde_json::Value) -> std::io::Result<()> {
        self.write_control(&serde_json::json!({
            "type": "configure",
            "payload": payload,
        }))
    }

    fn write_control(&mut self, value: &serde_json::Value) -> std::io::Result<()> {
        let stdin = self.stdin.as_mut().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "driver stdin is no longer available",
            )
        })?;
        serde_json::to_writer(&mut *stdin, value)?;
        stdin.write_all(b"\n")?;
        stdin.flush()
    }
}

impl Drop for DriverHandle {
    fn drop(&mut self) {
        // Mark shutdown as bridge-initiated so the watcher does not log a
        // SIGTERM-induced exit as "abnormal". The store happens before
        // stdin is closed (and therefore before any signal is sent) so
        // there is no window where the watcher could observe the child's
        // exit without the flag set.
        self.shutdown_initiated.store(true, Ordering::SeqCst);

        // Close stdin first. The SDK's main loop falls out on EOF, so for
        // well-behaved drivers this alone is enough to trigger a clean exit
        // without ever raising SIGTERM. The escalation below is the safety
        // net for drivers that ignore stdin EOF.
        drop(self.stdin.take());

        terminate_with_grace(&self.alive, self.child_pid, self.shutdown_grace);

        // Join the watcher first: its `child.wait()` is what reaps the kernel
        // corpse, so once the watcher returns the child stdout pipe is
        // guaranteed to have closed. The log-forwarder, which blocks on
        // `read_line` against that pipe, can then unblock immediately on EOF
        // — joining it second avoids an arbitrary wait while the child is
        // still in the kernel's signal-delivery window. Join errors are
        // swallowed because the inner threads only panic on logger writer
        // errors, which the logger already absorbs.
        if let Some(handle) = self.watcher.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.log_forwarder.take() {
            let _ = handle.join();
        }
    }
}

/// Errors returned by [`spawn_driver`] / [`spawn_driver_with_options`].
#[derive(Debug)]
pub enum SpawnError {
    /// `Command::spawn` itself failed (binary missing, permission denied, …).
    Spawn(std::io::Error),
    /// The driver process started but did not emit `hello` within the
    /// supplied timeout. Carries the configured budget so the `Display` impl
    /// can echo the value the caller passed in (tests use a much shorter
    /// timeout than [`HANDSHAKE_TIMEOUT`]). The child is killed before this
    /// is returned.
    HandshakeTimeout(Duration),
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
    /// Realistic only if the child crashed between emitting `hello` and the
    /// Bridge's ack write — a real cross-process boundary that can fail.
    HelloAckWrite(std::io::Error),
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(err) => write!(f, "driver プロセスの spawn に失敗しました: {err}"),
            Self::HandshakeTimeout(elapsed) => write!(
                f,
                "driver が {:.3} 秒以内に hello を送信しませんでした",
                elapsed.as_secs_f64()
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
            Self::HandshakeTimeout(_) | Self::HandshakeMissing | Self::IncompatibleSdk { .. } => {
                None
            }
        }
    }
}

/// Spawn a driver binary at `path`, complete the version handshake, and
/// return a [`DriverHandle`] tied to the live child.
///
/// `name` is the driver identifier (used for diagnostics and as the
/// `driver/<name>` layer string for forwarded log lines); `profile` is the
/// configuration blob the runtime will eventually forward to the driver —
/// handshake itself does not consume it, so the parameter is held only at
/// the API boundary.
///
/// On the failure path the child process is reaped before this function
/// returns — callers do not need to clean up zombies on `Err(_)`.
pub fn spawn_driver(
    name: impl Into<String>,
    path: impl Into<PathBuf>,
    profile: &serde_json::Value,
) -> Result<DriverHandle, SpawnError> {
    spawn_driver_with_options(name, path, profile, &SpawnOptions::default())
}

/// [`spawn_driver`] with explicit handshake timeout / shutdown-grace tuning.
/// Tests use a short timeout (typically 200 ms) and a short grace (typically
/// 200 ms) to keep the suite fast; production callers should keep using the
/// defaults via [`spawn_driver`].
pub fn spawn_driver_with_options(
    name: impl Into<String>,
    path: impl Into<PathBuf>,
    profile: &serde_json::Value,
    options: &SpawnOptions,
) -> Result<DriverHandle, SpawnError> {
    let _ = profile;
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

    let (hello_line, stdout_reader) =
        read_hello_line(stdout, options.handshake_timeout, &mut child)?;

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
        // The negative ack has been written; closing the child immediately
        // keeps the bridge non-blocking. Graceful exit on the driver side
        // is a lifecycle concern handled outside this module.
        kill_and_reap(&mut child);
        return Err(SpawnError::IncompatibleSdk {
            driver_version: hello.sdk_version,
            reason,
        });
    }

    // Hand the live child off to background workers and build the handle.
    let child_pid = child.id();
    let alive = Arc::new(AtomicBool::new(true));
    let shutdown_initiated = Arc::new(AtomicBool::new(false));

    let log_forwarder = spawn_log_forwarder(name.clone(), stdout_reader);
    let watcher = spawn_exit_watcher(
        name.clone(),
        child,
        Arc::clone(&alive),
        Arc::clone(&shutdown_initiated),
    );

    Ok(DriverHandle {
        name,
        path,
        sdk_version: hello.sdk_version,
        child_pid,
        stdin: Some(stdin),
        alive,
        shutdown_initiated,
        shutdown_grace: options.shutdown_grace,
        log_forwarder: Some(log_forwarder),
        watcher: Some(watcher),
    })
}
