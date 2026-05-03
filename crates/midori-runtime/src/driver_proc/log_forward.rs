use std::io::{BufRead, BufReader};
use std::process::ChildStdout;
use std::thread::{self, JoinHandle};

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
pub(super) fn spawn_log_forwarder(
    driver_name: String,
    mut reader: BufReader<ChildStdout>,
) -> JoinHandle<()> {
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
