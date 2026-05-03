use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::{lifecycle::kill_and_reap, SpawnError};

/// SDK majors the Bridge currently understands. The compatibility check
/// accepts any `hello.sdk_version` parseable as `MAJOR.MINOR.PATCH` whose
/// `MAJOR` is listed here. Bumping the SDK's binary layout extends this list
/// (or replaces it) — see `design/10-driver-plugin.md`「バージョン互換性」 for
/// the policy: layout changes happen on semver-major bumps only.
const ACCEPTED_SDK_MAJORS: &[u32] = &[0, 1];

/// JSON-tagged `hello` message read from the driver's stdout.
#[derive(Debug, serde::Deserialize)]
pub(super) struct HelloMessage {
    #[serde(rename = "type")]
    pub(super) message_type: String,
    pub(super) sdk_version: String,
}

/// JSON-tagged `hello_ack` message written to the driver's stdin.
#[derive(Debug, serde::Serialize)]
pub(super) struct HelloAckMessage<'a> {
    #[serde(rename = "type")]
    pub(super) message_type: &'a str,
    pub(super) compatible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reason: Option<&'a str>,
}

/// Internal channel payload used during handshake so the worker thread can
/// hand its `BufReader<ChildStdout>` back when it returns the first line.
/// Transferring ownership lets the post-handshake log forwarder pick up
/// reading from byte 0 of the next line without re-opening stdout.
pub(super) enum HandshakeReadOutcome {
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
pub(super) fn read_hello_line(
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

/// Result of [`is_sdk_compatible`]. Carried as a small enum so the caller
/// can build the `hello_ack` reason field and the `IncompatibleSdk` error
/// from the same source of truth.
pub(super) enum Compatibility {
    Compatible,
    Incompatible { reason: String },
}

impl Compatibility {
    pub(super) fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible)
    }

    pub(super) fn reason(&self) -> Option<&str> {
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
pub(super) fn is_sdk_compatible(version: &str) -> Compatibility {
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
pub(super) fn write_hello_ack(
    stdin: &mut ChildStdin,
    ack: &HelloAckMessage<'_>,
) -> std::io::Result<()> {
    serde_json::to_writer(&mut *stdin, ack)?;
    stdin.write_all(b"\n")?;
    stdin.flush()
}

/// Synthesize a `serde_json::Error` for the "valid JSON but wrong tag" case
/// so [`SpawnError::HandshakeMalformed`] can carry the diagnostic without a
/// dedicated variant.
pub(super) fn json_type_error(message: &str) -> serde_json::Error {
    // serde_json does not expose `Error::custom` publicly; round-trip
    // through `serde::de::Error` via a deserializer that produces our text.
    use serde::de::Error as _;
    serde_json::Error::custom(message)
}
