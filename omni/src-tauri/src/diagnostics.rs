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
