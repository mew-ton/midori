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
/// The accepted set is `MAJOR.MINOR.PATCH` triples of bare base-10 `u32`
/// integers whose `MAJOR` is listed in [`ACCEPTED_SDK_MAJORS`]. Each segment
/// is parsed with [`u32::from_str`], which rejects empty strings, leading
/// `-` (no negative `u32`) and surrounding whitespace; we additionally
/// reject a leading `+` sign so the protocol stays a strict subset of
/// `SemVer`'s MAJOR.MINOR.PATCH grammar (`SemVer` numeric identifiers are
/// unsigned and do not carry a sign character).
///
/// Pre-release / build metadata (`1.0.0-rc.1`, `1.0.0+build.7`) are out of
/// scope: such inputs fail the per-segment numeric parse and are reported
/// the same way as any other malformed segment.
///
/// The `Incompatible` reason names which segment failed and how, so the
/// driver-side log makes the mismatch obvious without the reader having to
/// reproduce the parse locally.
pub(super) fn is_sdk_compatible(version: &str) -> Compatibility {
    if version.is_empty() {
        return Compatibility::Incompatible {
            reason: format!("sdk_version `{version}` is empty"),
        };
    }
    let mut parts = version.split('.');
    let Some(major_str) = parts.next() else {
        // `str::split` always yields at least one element, so this is
        // unreachable in practice; keep the arm to make the iterator
        // contract explicit at the call site.
        return Compatibility::Incompatible {
            reason: format!("sdk_version `{version}` is empty"),
        };
    };
    let Some(minor_str) = parts.next() else {
        return Compatibility::Incompatible {
            reason: format!("sdk_version `{version}` is not in MAJOR.MINOR.PATCH form"),
        };
    };
    let Some(patch_str) = parts.next() else {
        return Compatibility::Incompatible {
            reason: format!("sdk_version `{version}` is not in MAJOR.MINOR.PATCH form"),
        };
    };
    if parts.next().is_some() {
        return Compatibility::Incompatible {
            reason: format!(
                "sdk_version `{version}` carries a trailing segment past MAJOR.MINOR.PATCH"
            ),
        };
    }
    // Each segment is parsed with `u32::from_str` (which rejects empty
    // strings, surrounding whitespace, and a leading `-`); the explicit
    // `starts_with('+')` guard then strips the only sign character that
    // `from_str` would otherwise accept, keeping the accepted shape at
    // exactly "one or more ASCII digits" per `SemVer` numeric identifiers.
    if major_str.starts_with('+') {
        return Compatibility::Incompatible {
            reason: format!(
                "sdk_version `{version}` has non-numeric major component `{major_str}`"
            ),
        };
    }
    let Ok(major) = major_str.parse::<u32>() else {
        return Compatibility::Incompatible {
            reason: format!(
                "sdk_version `{version}` has non-numeric major component `{major_str}`"
            ),
        };
    };
    if minor_str.starts_with('+') || minor_str.parse::<u32>().is_err() {
        return Compatibility::Incompatible {
            reason: format!(
                "sdk_version `{version}` has non-numeric minor component `{minor_str}`"
            ),
        };
    }
    if patch_str.starts_with('+') || patch_str.parse::<u32>().is_err() {
        return Compatibility::Incompatible {
            reason: format!(
                "sdk_version `{version}` has non-numeric patch component `{patch_str}`"
            ),
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

#[cfg(test)]
mod tests {
    use super::{is_sdk_compatible, Compatibility};

    /// Table-driven coverage of the MAJOR.MINOR.PATCH parser. The expected
    /// shape is paired with a fragment of the rejection reason so the test
    /// fails loudly if a future edit changes which segment a malformed
    /// input is blamed on (for example, accidentally reporting a 4-segment
    /// version as "non-numeric patch" instead of "trailing segment").
    enum Expected {
        Compatible,
        /// The rejection reason must contain this substring. The fragments
        /// are chosen to be specific to one rejection branch so a swap
        /// between branches is detectable.
        IncompatibleContains(&'static str),
    }

    #[test]
    fn it_should_validate_sdk_version_segments() {
        let cases: &[(&str, Expected)] = &[
            // Accepted: both currently-allowed majors.
            ("1.0.0", Expected::Compatible),
            ("0.1.0", Expected::Compatible),
            // Rejected: major outside the accepted set, but well-formed.
            (
                "99.0.0",
                Expected::IncompatibleContains("advertises major 99"),
            ),
            // Rejected: non-numeric minor and patch.
            (
                "1.foo.bar",
                Expected::IncompatibleContains("non-numeric minor"),
            ),
            // Rejected: empty minor segment between two dots.
            ("1..3", Expected::IncompatibleContains("non-numeric minor")),
            // Rejected: trailing segment past MAJOR.MINOR.PATCH.
            (
                "1.2.3.4",
                Expected::IncompatibleContains("trailing segment"),
            ),
            // Rejected: empty input.
            ("", Expected::IncompatibleContains("is empty")),
            // Rejected: leading whitespace — `u32::from_str` does not trim.
            (
                " 1.0.0",
                Expected::IncompatibleContains("non-numeric major"),
            ),
            // Rejected: leading minus sign — `u32` is unsigned.
            (
                "-1.0.0",
                Expected::IncompatibleContains("non-numeric major"),
            ),
            // Rejected: leading plus sign — explicitly stripped before parse
            // so the accepted shape stays "ASCII digits only".
            (
                "+1.0.0",
                Expected::IncompatibleContains("non-numeric major"),
            ),
            // Rejected: too few segments (regression guard for the existing
            // "must have at least three" path).
            ("1.0", Expected::IncompatibleContains("MAJOR.MINOR.PATCH")),
        ];

        for (input, expected) in cases {
            let result = is_sdk_compatible(input);
            match (expected, &result) {
                (Expected::Compatible, Compatibility::Compatible) => {}
                (
                    Expected::IncompatibleContains(needle),
                    Compatibility::Incompatible { reason },
                ) => {
                    assert!(
                        reason.contains(needle),
                        "input {input:?}: expected reason to contain {needle:?}, got {reason:?}"
                    );
                }
                (Expected::Compatible, Compatibility::Incompatible { reason }) => {
                    panic!("input {input:?}: expected Compatible, got Incompatible({reason:?})");
                }
                (Expected::IncompatibleContains(needle), Compatibility::Compatible) => {
                    panic!(
                        "input {input:?}: expected Incompatible containing {needle:?}, got Compatible"
                    );
                }
            }
        }
    }
}
