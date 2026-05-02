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

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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

/// SDK majors the Bridge currently understands. The compatibility check
/// accepts any `hello.sdk_version` parseable as `MAJOR.MINOR.PATCH` whose
/// `MAJOR` is listed here. Bumping the SDK's binary layout extends this list
/// (or replaces it) — see `design/10-driver-plugin.md`「バージョン互換性」 for
/// the policy: layout changes happen on semver-major bumps only.
const ACCEPTED_SDK_MAJORS: &[u32] = &[0, 1];

/// Poll interval used while waiting for the child to exit during the SIGTERM
/// → SIGKILL escalation window. Small enough that the worst-case overshoot
/// of the grace deadline is negligible (50 ms on a 3 s grace), large enough
/// that the busy-wait does not chew CPU.
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(50);

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

/// Internal channel payload used during handshake so the worker thread can
/// hand its `BufReader<ChildStdout>` back when it returns the first line.
/// Transferring ownership lets the post-handshake log forwarder pick up
/// reading from byte 0 of the next line without re-opening stdout.
enum HandshakeReadOutcome {
    Line {
        line: String,
        reader: BufReader<ChildStdout>,
    },
    Err(std::io::Error),
}

/// Read the driver's first stdout line under a wall-clock timeout, handing
/// the reader back to the caller so the post-handshake forwarder can keep
/// reading from byte 0 of line 2.
///
/// On the error paths the child is killed and reaped before returning so
/// the worker thread's `read_line` unblocks (otherwise `join` would deadlock
/// against the read still waiting on the live pipe).
fn read_hello_line(
    stdout: ChildStdout,
    timeout: Duration,
    child: &mut Child,
) -> Result<(String, BufReader<ChildStdout>), SpawnError> {
    let (tx, rx) = mpsc::channel::<HandshakeReadOutcome>();
    let reader_handle = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // Sender drops on return; the receiver sees Disconnected.
            }
            Ok(_) => {
                let _ = tx.send(HandshakeReadOutcome::Line { line, reader });
            }
            Err(err) => {
                let _ = tx.send(HandshakeReadOutcome::Err(err));
            }
        }
    });

    let result = match rx.recv_timeout(timeout) {
        Ok(HandshakeReadOutcome::Line { line, reader }) => Ok((line, reader)),
        Ok(HandshakeReadOutcome::Err(io_err)) => {
            kill_and_reap(child);
            Err(SpawnError::Spawn(io_err))
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            kill_and_reap(child);
            Err(SpawnError::HandshakeTimeout(timeout))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            kill_and_reap(child);
            Err(SpawnError::HandshakeMissing)
        }
    };
    let _ = reader_handle.join();
    result
}

/// Convert a non-JSON driver stdout line into the structured fields the
/// bridge logger expects. Pure function: no IO, no globals — the integration
/// test path verifies the call site is wired, and this unit-tests the
/// translation without needing a process boundary.
#[must_use]
pub fn non_json_line_to_log_event(driver_name: &str, line: &str) -> LogEvent {
    LogEvent {
        layer: format!("driver/{driver_name}"),
        device: None,
        message: line.trim_end_matches(['\r', '\n']).to_owned(),
    }
}

/// Output of [`non_json_line_to_log_event`]. Matches the field set the
/// `crate::logging::info` entrypoint takes.
#[derive(Debug, PartialEq, Eq)]
pub struct LogEvent {
    pub layer: String,
    pub device: Option<String>,
    pub message: String,
}

/// Drive the post-handshake stdout reader on its own thread, forwarding
/// each line into the bridge logger. Today every line is treated as a log
/// line — driver→bridge JSON control messages do not exist yet. When they
/// do, the dispatch will branch here.
fn spawn_log_forwarder(driver_name: String, mut reader: BufReader<ChildStdout>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => return, // EOF: child closed stdout
                Ok(_) => {
                    let event = non_json_line_to_log_event(&driver_name, &line);
                    crate::logging::info(&event.layer, event.device.as_deref(), &event.message);
                }
                Err(_err) => {
                    // Read errors mean the pipe is gone; the watcher will
                    // observe the child exit and flip `alive`. Bail.
                    return;
                }
            }
        }
    })
}

/// Wait on the child process from a dedicated thread so the bridge learns
/// about abnormal exits without blocking foreground work, and so the
/// [`Drop`]-side SIGTERM / SIGKILL escalation can synchronize against the
/// child's reap purely through the `alive` flag and the eventual thread
/// join. The watcher fully owns the `Child` for the duration of its life;
/// no other code ever calls `wait()` on it.
fn spawn_exit_watcher(
    driver_name: String,
    mut child: Child,
    alive: Arc<AtomicBool>,
    shutdown_initiated: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let status = child.wait();
        alive.store(false, Ordering::SeqCst);

        // If the bridge initiated the shutdown (i.e. `Drop` ran and sent
        // SIGTERM / SIGKILL), the resulting exit is **expected** even when
        // it manifests as a non-zero status (signal-terminated processes
        // report `ExitStatus::success() == false` on Unix). Suppressing
        // the abnormal-exit log on this path keeps "normal driver
        // shutdown" out of the bridge error stream.
        if shutdown_initiated.load(Ordering::SeqCst) {
            return;
        }

        match status {
            Ok(status) if status.success() => {
                // Graceful exit: nothing to log; the handle owner will
                // observe `is_alive() == false` if it cares.
            }
            Ok(status) => {
                crate::logging::error(
                    "bridge",
                    Some(&driver_name),
                    format_args!("driver subprocess exited abnormally: {status}"),
                );
            }
            Err(err) => {
                crate::logging::error(
                    "bridge",
                    Some(&driver_name),
                    format_args!("driver subprocess wait() failed: {err}"),
                );
            }
        }
    })
}

/// SIGTERM → grace → SIGKILL escalation. Synchronizes with the watcher
/// thread purely through the shared `alive` flag: the watcher sets it to
/// `false` after `child.wait()` returns, so polling it tells us when the
/// child has been reaped without us having to share the `Child` itself.
fn terminate_with_grace(alive: &AtomicBool, child_pid: u32, grace: Duration) {
    // Fast path: child already gone (graceful exit triggered by stdin EOF
    // closing the SDK loop, or pre-existing crash). Nothing to do.
    if !alive.load(Ordering::SeqCst) {
        return;
    }

    // Step 1: send SIGTERM. Best-effort; ESRCH (child raced to exit
    // between the load above and the kill below) is ignored inside
    // `send_sigterm`.
    send_sigterm(child_pid);

    // Step 2: poll the watcher's `alive` flag. The watcher sets it to
    // `false` after `child.wait()` returns, so a `false` reading means the
    // kernel has reaped the child and there is no SIGKILL work to do.
    let deadline = Instant::now() + grace;
    while alive.load(Ordering::SeqCst) {
        if Instant::now() >= deadline {
            // Step 3: SIGKILL escalation. Sending the signal via the cached
            // PID is sufficient — the watcher is parked in `child.wait()`
            // and will return as soon as the kernel delivers the SIGKILL.
            // The watcher then flips `alive` and exits, and the caller's
            // `Drop` joins it next, which synchronizes the reap.
            send_sigkill(child_pid);
            return;
        }
        thread::sleep(SHUTDOWN_POLL_INTERVAL);
    }
}

/// Send `SIGTERM` to `pid`. Best-effort: errors are intentionally ignored
/// (the most common error is ESRCH = the child already exited).
#[cfg(unix)]
fn send_sigterm(pid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    // `pid` came from `Child::id()` which returns a `u32` representing a
    // POSIX `pid_t`. PIDs are positive and well below `i32::MAX` on every
    // supported Unix kernel (Linux caps at `kernel.pid_max`, default 32768
    // / 4 194 304; macOS at 99 998), so the bit-pattern reinterpretation
    // via `cast_signed` is faithful — we just have to pick a signed
    // version explicitly so clippy's `cast_possible_wrap` lint stops
    // flagging the conversion. The workspace forbids `unsafe_code` so we
    // go through `nix`'s safe wrapper rather than `libc::kill`.
    let _ = kill(Pid::from_raw(pid.cast_signed()), Signal::SIGTERM);
}

#[cfg(not(unix))]
fn send_sigterm(_pid: u32) {
    // Windows lacks POSIX signals; the equivalent shutdown path requires
    // `TerminateProcess`, which has different semantics (no graceful window).
    // Until the lifecycle module is taught to switch between models, this
    // build target relies on the child exiting via stdin EOF, and
    // `send_sigkill` below is also a no-op.
}

/// Send `SIGKILL` to `pid`. Best-effort — ESRCH is ignored.
#[cfg(unix)]
fn send_sigkill(pid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    // Same `pid_t` reasoning as `send_sigterm` — see that function for
    // the cast / `unsafe_code = "forbid"` rationale.
    let _ = kill(Pid::from_raw(pid.cast_signed()), Signal::SIGKILL);
}

#[cfg(not(unix))]
fn send_sigkill(_pid: u32) {
    // See `send_sigterm` for the Windows scope note.
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

/// Synthesize a `serde_json::Error` for the "valid JSON but wrong tag" case
/// so [`SpawnError::HandshakeMalformed`] can carry the diagnostic without a
/// dedicated variant.
fn json_type_error(message: &str) -> serde_json::Error {
    // serde_json does not expose `Error::custom` publicly; round-trip
    // through `serde::de::Error` via a deserializer that produces our text.
    use serde::de::Error as _;
    serde_json::Error::custom(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_strip_trailing_newline_when_converting_non_json_line() {
        let event = non_json_line_to_log_event("dummy", "hello world\n");
        assert_eq!(event.layer, "driver/dummy");
        assert_eq!(event.device, None);
        assert_eq!(event.message, "hello world");
    }

    #[test]
    fn it_should_strip_trailing_crlf_when_converting_non_json_line() {
        let event = non_json_line_to_log_event("midi", "boom\r\n");
        assert_eq!(event.message, "boom");
    }

    #[test]
    fn it_should_preserve_internal_whitespace_when_converting_non_json_line() {
        let event = non_json_line_to_log_event("osc", "  spaced  message  ");
        assert_eq!(event.message, "  spaced  message  ");
    }

    #[test]
    fn it_should_use_driver_layer_prefix_in_log_event() {
        let event = non_json_line_to_log_event("my-driver", "x");
        assert_eq!(event.layer, "driver/my-driver");
    }
}
