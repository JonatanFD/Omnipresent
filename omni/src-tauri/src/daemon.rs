//! The embedded daemon and its lifecycle.
//!
//! The daemon runs on a background thread of this process, which means the
//! window has to be able to stop it and start it again — and those two have to
//! be reliable in sequence, because "Stop, then Start" is what a user does when
//! anything looks wrong.
//!
//! Stopping is therefore synchronous: it asks the daemon to stop and then waits
//! for the thread to actually end. The daemon unwinds through two grace periods
//! before `run` returns, so for a few seconds after the request it is still
//! holding its UDP socket and its IPC endpoint. Starting during that window
//! cannot work — on Windows the named pipe is created with
//! `FILE_FLAG_FIRST_PIPE_INSTANCE` and the old instance is still there — so the
//! only correct thing is to wait rather than to refuse.
//!
//! Starting is not the same as owning. If another daemon already serves the
//! socket, ours fails to bind and stands down, and the window talks to the one
//! that is there. That is a normal outcome, not a failure to report.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use omni_runtime::ipc::Request;
use omni_runtime::ipc_transport::connect_blocking;
use omni_runtime::Paths;
use tauri::{AppHandle, Emitter, State};

/// Emitted when the embedded daemon thread ends, whatever the reason.
pub const EXITED_EVENT: &str = "daemon://exited";

/// How long to wait for the daemon to finish unwinding after being asked to
/// stop. The daemon itself allows two two-second grace periods, so this has to
/// exceed their sum or a legitimate shutdown would look like a hang.
const STOP_TIMEOUT: Duration = Duration::from_secs(8);

/// How often to look at whether the thread has ended while waiting for it.
const STOP_POLL: Duration = Duration::from_millis(50);

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
#[derive(Default)]
pub struct Supervisor {
    /// The running thread, kept so stopping can wait for it to finish rather
    /// than guess. A flag alone cannot express "winding down".
    thread: Mutex<Option<JoinHandle<()>>>,
    /// Set for as long as the daemon is inside `run`, including while it
    /// unwinds. Read without taking the lock, so status queries never block
    /// behind a stop that is in progress.
    running: Arc<AtomicBool>,
}

impl Supervisor {
    /// Starts the daemon unless one of ours is already running.
    ///
    /// Succeeds when a daemon of ours is already up: the caller asked for a
    /// running daemon and there is one. Returning an error there turned an
    /// ordinary second click into a message about something the user cannot act
    /// on.
    pub fn start(&self, app: AppHandle) -> Result<(), String> {
        let mut slot = self
            .thread
            .lock()
            .map_err(|_| "the daemon supervisor is unusable".to_string())?;

        // Reap a thread that has already finished, so its handle does not make
        // a stopped daemon look like a running one.
        if slot.as_ref().is_some_and(|handle| handle.is_finished()) {
            if let Some(handle) = slot.take() {
                let _ = handle.join();
            }
        }
        if slot.is_some() {
            return Ok(());
        }

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        *slot = Some(std::thread::spawn(move || {
            let outcome = omni_runtime::run();
            running.store(false, Ordering::SeqCst);

            let reason = outcome.err().map(|error| error.to_string());
            // A failure to start usually means someone else got there first, so
            // check before calling it an error the user needs to see.
            let stood_down = reason.is_some() && daemon_is_reachable();
            let _ = app.emit(EXITED_EVENT, Exit { reason, stood_down });
        }));

        Ok(())
    }

    /// Stops the daemon and waits for it to be gone.
    ///
    /// Waiting is the point. `run` returns only after the daemon has released
    /// its UDP socket and its IPC endpoint, and a Start issued before that
    /// cannot bind either of them. Returning early would make the next Start
    /// fail for reasons the user could not see or fix.
    pub fn stop(&self) -> Result<(), String> {
        let mut slot = self
            .thread
            .lock()
            .map_err(|_| "the daemon supervisor is unusable".to_string())?;

        let Some(handle) = slot.take() else {
            // Nothing of ours is running. A daemon started elsewhere is still
            // asked to stop, which is what the CLI would do.
            return request_stop();
        };

        request_stop()?;

        let deadline = Instant::now() + STOP_TIMEOUT;
        while !handle.is_finished() {
            if Instant::now() >= deadline {
                // Put it back: it is still running, and the next Start must not
                // spawn a second one alongside it.
                *slot = Some(handle);
                return Err("the daemon did not stop in time".into());
            }
            std::thread::sleep(STOP_POLL);
        }
        let _ = handle.join();
        Ok(())
    }

    /// Whether the daemon this app is talking to is the one inside it.
    pub fn is_embedded(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Asks whatever daemon is listening to shut down.
fn request_stop() -> Result<(), String> {
    match crate::ipc::request(Request::Stop) {
        Ok(_) => Ok(()),
        // Nothing listening is the state the caller wanted anyway.
        Err(message) if message.contains("not running") => Ok(()),
        Err(message) => Err(message),
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

/// Stops the daemon and waits for it to be gone, so the Start that usually
/// follows has a socket to bind.
#[tauri::command]
pub fn daemon_stop(supervisor: State<'_, Supervisor>) -> Result<(), String> {
    supervisor.stop()
}

/// Lets the window say whether it is driving a daemon of its own or one that was
/// already running, because Quit means something different in each case.
#[tauri::command]
pub fn daemon_embedded(supervisor: State<'_, Supervisor>) -> bool {
    supervisor.is_embedded()
}

/// Response to a `Stop` that reports success even when nothing was listening.
/// Used by the tray, which stops the daemon on Quit.
pub fn stop_quietly(supervisor: &Supervisor) {
    let _ = supervisor.stop();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_supervisor_owns_no_daemon() {
        let supervisor = Supervisor::default();

        assert!(!supervisor.is_embedded());
        assert!(supervisor.thread.lock().unwrap().is_none());
    }

    #[test]
    fn stopping_when_nothing_runs_is_not_an_error() {
        // Stop is what a user reaches for when things look wrong, including when
        // the daemon is already gone. That must not report a failure.
        assert!(Supervisor::default().stop().is_ok());
    }

    #[test]
    fn stopping_leaves_the_slot_free_for_the_next_start() {
        // The whole point of the fix: after Stop returns, Start must be able to
        // claim the slot. It used to be refused for as long as the old daemon
        // took to unwind, and nothing retried.
        let supervisor = Supervisor::default();

        supervisor.stop().expect("stopping nothing succeeds");

        assert!(supervisor.thread.lock().unwrap().is_none());
    }

    #[test]
    fn the_stop_timeout_outlasts_the_daemons_own_grace_periods() {
        // The daemon unwinds through two two-second graces before `run` returns.
        // A shorter wait here would report a hang on an ordinary shutdown.
        assert!(STOP_TIMEOUT >= Duration::from_secs(5));
    }

    #[test]
    fn a_clean_exit_carries_no_reason() {
        let json = serde_json::to_value(Exit {
            reason: None,
            stood_down: false,
        })
        .expect("an exit serialises");

        assert!(json["reason"].is_null());
        assert_eq!(json["stood_down"], false);
    }

    #[test]
    fn standing_down_is_reported_separately_from_the_reason() {
        let json = serde_json::to_value(Exit {
            reason: Some("QUIC endpoint: address already in use".into()),
            stood_down: true,
        })
        .expect("an exit serialises");

        assert_eq!(json["stood_down"], true);
        assert!(json["reason"]
            .as_str()
            .is_some_and(|r| r.contains("address already in use")));
    }
}
