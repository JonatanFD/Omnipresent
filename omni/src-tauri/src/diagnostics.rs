//! `omni doctor`, for the window.
//!
//! The checks are the daemon's own (`omni_runtime::doctor`), run in this
//! process. That is available here and nowhere else: the native clients speak
//! only IPC and the protocol has no `Doctor` request, but this app links the
//! runtime, so it can ask the same questions the CLI asks without inventing a
//! message for them.
//!
//! It matters more here than in a terminal. Someone who never granted the
//! Accessibility permission gets a machine that can only be driven, never
//! drive — and the only hint is a "Target only" badge that does not say why.

use omni_runtime::ipc::{Request, Response};
use omni_runtime::Paths;

use crate::ipc::request;

/// One environment requirement and whether it is met.
#[derive(Clone, serde::Serialize)]
pub struct CheckInfo {
    pub name: String,
    pub ok: bool,
    /// What was found — and, when not ok, how to fix it.
    pub detail: String,
}

impl CheckInfo {
    fn failed(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            ok: false,
            detail: detail.into(),
        }
    }
}

impl From<omni_runtime::doctor::Check> for CheckInfo {
    fn from(check: omni_runtime::doctor::Check) -> Self {
        Self {
            name: check.name.to_string(),
            ok: check.ok,
            detail: check.detail,
        }
    }
}

/// Runs every check, in the order they should be read.
#[tauri::command]
pub fn daemon_doctor() -> Result<Vec<CheckInfo>, String> {
    let paths = Paths::resolve().map_err(|e| e.to_string())?;

    let mut checks: Vec<CheckInfo> = omni_runtime::doctor::run_checks(&paths)
        .into_iter()
        .map(CheckInfo::from)
        .collect();
    checks.push(capture_check());

    Ok(checks)
}

/// The daemon's own view. A permission granted after it started does not reach
/// it, so a daemon that came up without one keeps running as target-only until
/// it is restarted — which the environment checks above cannot see.
fn capture_check() -> CheckInfo {
    match request(Request::Status) {
        Ok(Response::Status(status)) if status.capturing => CheckInfo {
            name: "daemon".into(),
            ok: true,
            detail: "running, input capture active".into(),
        },
        Ok(Response::Status(_)) => CheckInfo::failed(
            "daemon",
            "running, but capture is off — this machine can only be driven. \
             Grant the permission above, then stop and start the daemon in General.",
        ),
        Ok(_) => CheckInfo::failed("daemon", "answered something other than a status"),
        Err(message) => CheckInfo::failed("daemon", message),
    }
}

/// The daemon's own log, as the window shows it.
#[derive(Clone, serde::Serialize)]
pub struct DaemonLog {
    /// Where the file is, so the user can open it themselves.
    pub path: String,
    /// The most recent lines, oldest first. Empty when there is no log yet.
    pub lines: Vec<String>,
}

/// How much of the end of the log to read. A daemon that has been running for
/// weeks has a long log, and only the end of it explains what just happened.
const LOG_TAIL_BYTES: u64 = 64 * 1024;
const LOG_TAIL_LINES: usize = 200;

/// Reads the tail of the daemon's log.
///
/// This is the only place the app can say *why* a daemon refused to start.
/// Without it a failure looks the same from the window whatever caused it —
/// "not running" — and the one machine that needs an answer is the one whose
/// owner cannot read a log file they do not know exists.
#[tauri::command]
pub fn daemon_log() -> Result<DaemonLog, String> {
    use std::io::{Read, Seek, SeekFrom};

    let paths = Paths::resolve().map_err(|e| e.to_string())?;
    let path = paths.log_file();
    let display = path.display().to_string();

    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        // No log is not an error: the daemon may never have run on this machine.
        Err(_) => {
            return Ok(DaemonLog {
                path: display,
                lines: Vec::new(),
            });
        }
    };

    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if size > LOG_TAIL_BYTES {
        file.seek(SeekFrom::End(-(LOG_TAIL_BYTES as i64)))
            .map_err(|e| e.to_string())?;
    }
    let mut text = String::new();
    // Lossy, and deliberately: a log that is not quite UTF-8 — a truncated
    // first line after seeking, say — is still worth reading.
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    text.push_str(&String::from_utf8_lossy(&bytes));

    Ok(DaemonLog {
        path: display,
        lines: tail_lines(&text, LOG_TAIL_LINES),
    })
}

/// The last `count` non-empty lines, oldest first.
///
/// The first line is dropped when the read started mid-file, because seeking by
/// bytes lands in the middle of one and half a log line reads as nonsense.
fn tail_lines(text: &str, count: usize) -> Vec<String> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(count);
    lines[start..].iter().map(|l| l.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use omni_runtime::doctor::Check;

    #[test]
    fn a_runtime_check_keeps_its_meaning_when_converted() {
        let info = CheckInfo::from(Check::ok("accessibility permission", "granted"));

        assert_eq!(info.name, "accessibility permission");
        assert!(info.ok);
        assert_eq!(info.detail, "granted");
    }

    #[test]
    fn a_failed_runtime_check_keeps_its_fix_instructions() {
        // The detail is the only place a user is told what to do about it, so it
        // has to survive the trip to the window intact.
        let info = CheckInfo::from(Check::failed(
            "uinput",
            "/dev/uinput is not writable — add yourself to the input group",
        ));

        assert!(!info.ok);
        assert!(info.detail.contains("input group"));
    }

    #[test]
    fn a_check_serialises_to_the_shape_the_window_reads() {
        let json = serde_json::to_value(CheckInfo::failed("daemon", "not running"))
            .expect("a check serialises");

        assert_eq!(json["name"], "daemon");
        assert_eq!(json["ok"], false);
        assert_eq!(json["detail"], "not running");
    }

    #[test]
    fn the_log_tail_keeps_the_most_recent_lines_in_order() {
        // Oldest first, because a log read bottom-up is unreadable.
        let text = (1..=10)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");

        let tail = tail_lines(&text, 3);

        assert_eq!(tail, vec!["line 8", "line 9", "line 10"]);
    }

    #[test]
    fn a_short_log_is_returned_whole() {
        assert_eq!(tail_lines("only one", 200), vec!["only one"]);
    }

    #[test]
    fn blank_lines_are_dropped() {
        // tracing writes a trailing newline, and a blank row in the pane is
        // just noise the user has to scroll past.
        assert_eq!(tail_lines("a\n\n\nb\n", 200), vec!["a", "b"]);
    }

    #[test]
    fn an_empty_log_yields_nothing_rather_than_one_empty_line() {
        assert!(tail_lines("", 200).is_empty());
        assert!(tail_lines("\n\n", 200).is_empty());
    }

    #[test]
    fn reading_the_log_never_fails_when_there_is_none() {
        // A machine where the daemon has never run has no log, and that is not
        // an error the window should show — it is the normal first-launch state.
        let log = daemon_log().expect("a missing log is not an error");

        assert!(!log.path.is_empty());
    }

    #[test]
    fn the_capture_check_fails_when_no_daemon_answers() {
        // No daemon is listening in a test process, which is the same shape as a
        // daemon that has been stopped: reported as a failed check, never a
        // panic and never a silent pass.
        let check = capture_check();

        assert_eq!(check.name, "daemon");
        assert!(!check.ok);
        assert!(!check.detail.is_empty());
    }
}
