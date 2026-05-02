use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Poll interval used while waiting for the child to exit during the SIGTERM
/// → SIGKILL escalation window. Small enough that the worst-case overshoot
/// of the grace deadline is negligible (50 ms on a 3 s grace), large enough
/// that the busy-wait does not chew CPU.
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Wait on the child process from a dedicated thread so the bridge learns
/// about abnormal exits without blocking foreground work, and so the
/// [`Drop`]-side SIGTERM / SIGKILL escalation can synchronize against the
/// child's reap purely through the `alive` flag and the eventual thread
/// join. The watcher fully owns the `Child` for the duration of its life;
/// no other code ever calls `wait()` on it.
pub(super) fn spawn_exit_watcher(
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
pub(super) fn terminate_with_grace(alive: &AtomicBool, child_pid: u32, grace: Duration) {
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

/// Kill the child and wait for it so callers do not leak zombies on the
/// error paths. Errors during kill / wait are intentionally swallowed: the
/// outer `Err(SpawnError::…)` already carries the user-visible cause and
/// the cleanup is best-effort.
pub(super) fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
