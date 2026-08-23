//! The embedded daemon and its lifecycle.
//!
//! The daemon runs on a background thread of this process, which means the
//! window has to be able to *start* it as well as stop it. Stopping it with no
//! way back would leave the app running with nothing to talk to, and the only
//! remedy would be quitting and launching again.
//!
//! Starting the daemon is not the same as owning it. If another one is already
//! serving the socket — one from `omni start`, or a second copy of this app —
//! ours fails to bind and stands down, and the window talks to the one that is
//! there. That is a normal outcome, not a failure to report.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use omni_runtime::ipc_transport::connect_blocking;
use omni_runtime::Paths;
use tauri::{AppHandle, Emitter, State};

/// Emitted when the embedded daemon thread ends, whatever the reason.
pub const EXITED_EVENT: &str = "daemon://exited";

/// Why the embedded daemon stopped.
#[derive(Clone, serde::Serialize)]
pub struct Exit {
    /// The failure that ended it, or `None` when it was asked to stop.
    pub reason: Option<String>,
    /// True when another daemon owns the socket, so ours was never needed.
    /// The window is still fully working in this case.
    pub stood_down: bool,
}

/// Owns the embedded daemon thread.
///
/// The thread is tracked with a flag rather than a `JoinHandle` because nothing
/// ever joins it: the daemon runs until it is told to stop, and the thread ends
/// on its own when it does.
#[derive(Default)]
pub struct Supervisor {
    running: Arc<AtomicBool>,
}

impl Supervisor {
    /// Starts the daemon on a background thread, unless one of ours is already
    /// running.
    pub fn start(&self, app: AppHandle) -> Result<(), String> {
        // `swap` claims the slot and tests it in one step, so two Start actions
        // arriving together cannot both spawn a daemon.
        if self.running.swap(true, Ordering::SeqCst) {
            return Err("the daemon is already running in this app".into());
        }

        let running = self.running.clone();
        std::thread::spawn(move || {
            let outcome = omni_runtime::run();
            running.store(false, Ordering::SeqCst);

            let reason = outcome.err().map(|error| error.to_string());
            // A failure to start usually means someone else got there first, so
            // check before calling it an error the user needs to see.
            let stood_down = reason.is_some() && daemon_is_reachable();
            let _ = app.emit(EXITED_EVENT, Exit { reason, stood_down });
        });

        Ok(())
    }

    /// Whether the daemon this app is talking to is the one inside it.
    pub fn is_embedded(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Whether *some* daemon is serving the local socket — ours or another one.
fn daemon_is_reachable() -> bool {
    Paths::resolve()
        .ok()
        .filter(|paths| connect_blocking(paths).is_ok())
        .is_some()
}

#[tauri::command]
pub fn daemon_start(app: AppHandle, supervisor: State<'_, Supervisor>) -> Result<(), String> {
    supervisor.start(app)
}

/// Lets the window say whether it is driving a daemon of its own or one that was
/// already running, because Quit means something different in each case.
#[tauri::command]
pub fn daemon_embedded(supervisor: State<'_, Supervisor>) -> bool {
    supervisor.is_embedded()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_supervisor_owns_no_daemon() {
        assert!(!Supervisor::default().is_embedded());
    }

    #[test]
    fn the_running_flag_is_claimed_once() {
        // Stands in for `start`, whose spawning half needs an `AppHandle`. What
        // matters here is that the claim is atomic: the first caller takes the
        // slot and every later one is turned away until the thread releases it.
        let supervisor = Supervisor::default();

        assert!(!supervisor.running.swap(true, Ordering::SeqCst));
        assert!(supervisor.is_embedded());
        assert!(supervisor.running.swap(true, Ordering::SeqCst));

        supervisor.running.store(false, Ordering::SeqCst);
        assert!(!supervisor.is_embedded());
    }

    #[test]
    fn a_clean_exit_carries_no_reason() {
        let exit = Exit {
            reason: None,
            stood_down: false,
        };
        let json = serde_json::to_value(&exit).expect("an exit serialises");

        assert!(json["reason"].is_null());
        assert_eq!(json["stood_down"], false);
    }

    #[test]
    fn standing_down_is_reported_separately_from_the_reason() {
        // Both fields travel: the window needs the reason to show when the
        // daemon really is gone, and the flag to stay quiet when it is not.
        let exit = Exit {
            reason: Some("QUIC endpoint: address already in use".into()),
            stood_down: true,
        };
        let json = serde_json::to_value(&exit).expect("an exit serialises");

        assert_eq!(json["stood_down"], true);
        assert!(json["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("address already in use")));
    }
}
