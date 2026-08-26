//! The bridge between the window and the daemon.
//!
//! Every command here is a thin pass-through to the daemon's local IPC surface
//! (`crates/omni-runtime/src/ipc.rs`): serialise a `Request`, read one `Response`
//! line back. No decision the daemon owns — trust, layout maths, fingerprinting —
//! is ever taken here.
//!
//! The daemon runs inside this process (see `lib.rs`), but this still talks to it
//! over the same local socket the CLI uses. That is deliberate: it keeps the core
//! untouched, and it means the window works just as well against a daemon that was
//! already running when the app started.

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

use omni_runtime::ipc::{
    Event, LayoutInfo, ModifierInfo, PeerInfo, PendingInfo, Request, Response, StatusInfo,
    PROTOCOL_VERSION,
};
use omni_runtime::ipc_transport::connect_blocking;
use omni_runtime::Paths;
use tauri::{AppHandle, Emitter};

/// Pushed to the window whenever the daemon reports new state.
pub const STATUS_EVENT: &str = "daemon://status";
/// Pushed when the subscription drops, so the window can show it is out of touch.
pub const DISCONNECTED_EVENT: &str = "daemon://disconnected";

/// Sends one request and reads the single response line it gets back.
///
/// Visible to the crate because the tray answers connection requests too: a
/// waiting peer has to be acceptable without reopening the window.
///
/// A read deadline guards against a daemon that accepted the connection but
/// stopped responding: without it, a wedged daemon would leave the window
/// spinning forever on a command that will never come back.
pub(crate) fn request(req: Request) -> Result<Response, String> {
    let paths = Paths::resolve().map_err(|e| e.to_string())?;
    let mut stream =
        connect_blocking(&paths).map_err(|_| "the daemon is not running".to_string())?;

    // Cap how long we wait for an answer. The daemon answers every request in
    // well under a second; anything longer means it is stuck or gone.
    set_read_deadline(&stream, READ_TIMEOUT);

    let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| format!("could not reach the daemon: {e}"))?;

    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .map_err(|e| format!("the daemon did not answer: {e}"))?;
    if reply.is_empty() {
        return Err("the daemon closed the connection without answering".into());
    }

    let response: Response =
        serde_json::from_str(reply.trim_end()).map_err(|e| format!("bad reply: {e}"))?;
    if let Response::Error { message } = response {
        return Err(message);
    }
    Ok(response)
}

/// How long `request` waits for the daemon to answer before giving up.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Applies a read deadline to the IPC stream, where the platform allows it.
/// Unix-domain sockets support `set_read_timeout`; a Windows named pipe opened
/// as a file does not, so there the deadline is a best-effort no-op.
#[cfg(unix)]
fn set_read_deadline(stream: &omni_runtime::ipc_transport::IpcClient, timeout: Duration) {
    // `set_read_timeout` on a blocking Unix socket is safe and takes effect on
    // the next `read`.
    let _ = stream.set_read_timeout(Some(timeout));
}

#[cfg(windows)]
fn set_read_deadline(_stream: &omni_runtime::ipc_transport::IpcClient, _timeout: Duration) {
    // A named pipe opened as `std::fs::File` has no per-handle read timeout.
    // The daemon answers promptly in practice; a wedged daemon is rare enough
    // that blocking here is the lesser evil than pulling in overlapped I/O for
    // every one-shot CLI call.
}

/// Rejects a response that is not the variant the caller asked for.
fn unexpected<T>(what: &str) -> Result<T, String> {
    Err(format!("the daemon answered something other than {what}"))
}

// MARK: - Commands

#[tauri::command]
pub fn daemon_status() -> Result<StatusInfo, String> {
    match request(Request::Status)? {
        Response::Status(status) => Ok(status),
        _ => unexpected("a status"),
    }
}

/// Checks the daemon speaks a protocol this build understands. A daemon newer
/// than the app is reported rather than guessed at, so the window can tell the
/// user to update instead of misbehaving.
#[tauri::command]
pub fn daemon_hello() -> Result<DaemonVersion, String> {
    match request(Request::Hello)? {
        Response::Hello {
            protocol_version,
            daemon_version,
        } => Ok(DaemonVersion {
            compatible: protocol_version <= PROTOCOL_VERSION,
            protocol_version,
            daemon_version,
        }),
        _ => unexpected("a hello"),
    }
}

#[derive(serde::Serialize)]
pub struct DaemonVersion {
    pub protocol_version: u32,
    pub daemon_version: String,
    /// False when the daemon is newer than this app understands.
    pub compatible: bool,
}

#[tauri::command]
pub fn peer_connect(host: String) -> Result<(), String> {
    request(Request::Connect { host }).map(|_| ())
}

#[tauri::command]
pub fn peer_disconnect(host: String) -> Result<(), String> {
    request(Request::Disconnect { host }).map(|_| ())
}

#[tauri::command]
pub fn peer_accept(selector: String) -> Result<(), String> {
    request(Request::Accept { selector }).map(|_| ())
}

#[tauri::command]
pub fn peer_reject(selector: String) -> Result<(), String> {
    request(Request::Reject { selector }).map(|_| ())
}

#[tauri::command]
pub fn peer_list() -> Result<Vec<PeerInfo>, String> {
    match request(Request::Peers)? {
        Response::Peers { peers } => Ok(peers),
        _ => unexpected("a peer list"),
    }
}

#[tauri::command]
pub fn peer_remove(selector: String) -> Result<(), String> {
    request(Request::RemovePeer { selector }).map(|_| ())
}

#[tauri::command]
pub fn layout_list() -> Result<Vec<LayoutInfo>, String> {
    match request(Request::Layout {
        host: None,
        edge: None,
    })? {
        Response::Layout { placements } => Ok(placements),
        _ => unexpected("a layout"),
    }
}

#[tauri::command]
pub fn layout_set(host: String, edge: String) -> Result<(), String> {
    request(Request::Layout {
        host: Some(host),
        edge: Some(edge),
    })
    .map(|_| ())
}

#[tauri::command]
pub fn modifiers_list() -> Result<Vec<ModifierInfo>, String> {
    match request(Request::Modifiers {
        host: None,
        swap: None,
    })? {
        Response::Modifiers { swaps } => Ok(swaps),
        _ => unexpected("a modifier list"),
    }
}

#[tauri::command]
pub fn modifiers_set(host: String, swap: String) -> Result<(), String> {
    request(Request::Modifiers {
        host: Some(host),
        swap: Some(swap),
    })
    .map(|_| ())
}

/// Clipboard sharing stays opt-in: this only forwards the user's choice, and the
/// daemon is what actually reads or applies a clipboard.
#[tauri::command]
pub fn clipboard_set(enabled: bool) -> Result<(), String> {
    request(Request::Clipboard { enabled }).map(|_| ())
}

// MARK: - Live updates

/// Holds a `Subscribe` connection open and forwards each pushed snapshot to the
/// window, reconnecting when it drops.
///
/// The daemon pushes only when something changes, so an idle app does no work —
/// which is the point of subscribing instead of polling `Status` on a timer.
pub fn spawn_subscription(app: AppHandle) {
    std::thread::spawn(move || {
        // Which requests the user has already been told about, so reconnecting
        // or an unrelated state change does not announce the same peer twice.
        let mut announced: HashSet<String> = HashSet::new();

        loop {
            match subscribe_once(&app, &mut announced) {
                Ok(()) => {}
                Err(_) => {
                    let _ = app.emit(DISCONNECTED_EVENT, ());
                }
            }
            // The daemon may still be starting, or may have been stopped on
            // purpose. Retrying on a slow beat costs nothing and recovers on
            // its own.
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

fn subscribe_once(app: &AppHandle, announced: &mut HashSet<String>) -> Result<(), String> {
    let paths = Paths::resolve().map_err(|e| e.to_string())?;
    let mut stream = connect_blocking(&paths).map_err(|e| e.to_string())?;

    let mut line = serde_json::to_string(&Request::Subscribe).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;

    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        // Unknown event variants are skipped rather than fatal, so a newer daemon
        // adding one does not knock the window offline.
        if let Ok(Event::Status(status)) = serde_json::from_str::<Event>(&line) {
            // The tray and the notification come first: they are what reaches a
            // user whose window is closed, which is the usual case.
            crate::tray::sync(app, &status);
            announce_new_requests(app, &status, announced);
            let _ = app.emit(STATUS_EVENT, status);
        }
    }
    Ok(())
}

/// Notifies about requests that have appeared since the last snapshot.
///
/// Split out and given its set explicitly so the "only once per peer" rule can
/// be tested without a daemon or a desktop.
fn announce_new_requests(app: &AppHandle, status: &StatusInfo, announced: &mut HashSet<String>) {
    for request in new_requests(status, announced) {
        crate::notify::incoming_request(app, &request.host);
    }
}

/// The waiting requests not yet announced, updating `announced` to match the
/// snapshot — so a request that is answered and later arrives again is
/// announced again, while one that merely persists is not.
fn new_requests(status: &StatusInfo, announced: &mut HashSet<String>) -> Vec<PendingInfo> {
    let waiting: HashSet<String> = status
        .pending
        .iter()
        .map(|request| request.fingerprint.clone())
        .collect();

    announced.retain(|fingerprint| waiting.contains(fingerprint));

    let fresh: Vec<PendingInfo> = status
        .pending
        .iter()
        .filter(|request| !announced.contains(&request.fingerprint))
        .cloned()
        .collect();

    announced.extend(fresh.iter().map(|request| request.fingerprint.clone()));
    fresh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(pending: &[(&str, &str)]) -> StatusInfo {
        StatusInfo {
            fingerprint: "ours".into(),
            port: 4733,
            capturing: true,
            clipboard_sharing: false,
            sessions: Vec::new(),
            pending: pending
                .iter()
                .map(|(host, fingerprint)| PendingInfo {
                    host: (*host).to_string(),
                    fingerprint: (*fingerprint).to_string(),
                })
                .collect(),
            peers: Vec::new(),
            placements: Vec::new(),
            modifier_swaps: Vec::new(),
        }
    }

    #[test]
    fn a_first_request_is_announced() {
        let mut announced = HashSet::new();

        let fresh = new_requests(&snapshot(&[("studio", "aa")]), &mut announced);

        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].host, "studio");
    }

    #[test]
    fn a_request_that_is_still_waiting_is_not_announced_again() {
        // The daemon pushes a whole snapshot on every change, so the same
        // pending request arrives many times over. Announcing each one would
        // notify repeatedly for a single peer.
        let mut announced = HashSet::new();
        let status = snapshot(&[("studio", "aa")]);

        new_requests(&status, &mut announced);
        let second = new_requests(&status, &mut announced);

        assert!(second.is_empty());
    }

    #[test]
    fn only_the_peer_that_is_new_is_announced() {
        let mut announced = HashSet::new();
        new_requests(&snapshot(&[("studio", "aa")]), &mut announced);

        let fresh = new_requests(
            &snapshot(&[("studio", "aa"), ("laptop", "bb")]),
            &mut announced,
        );

        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].host, "laptop");
    }

    #[test]
    fn a_peer_that_asks_again_after_being_answered_is_announced_again() {
        // Accepting or rejecting clears the request. If the same machine asks
        // later, that is a new decision to make and has to be surfaced.
        let mut announced = HashSet::new();
        new_requests(&snapshot(&[("studio", "aa")]), &mut announced);

        new_requests(&snapshot(&[]), &mut announced);
        let again = new_requests(&snapshot(&[("studio", "aa")]), &mut announced);

        assert_eq!(again.len(), 1);
    }

    #[test]
    fn an_empty_snapshot_forgets_what_was_announced() {
        let mut announced = HashSet::new();
        new_requests(&snapshot(&[("studio", "aa")]), &mut announced);

        new_requests(&snapshot(&[]), &mut announced);

        assert!(announced.is_empty());
    }

    #[test]
    fn peers_are_told_apart_by_fingerprint_not_name() {
        // Two machines can report the same host name; the fingerprint is what
        // actually identifies one. Keying on the name would silence the second.
        let mut announced = HashSet::new();
        new_requests(&snapshot(&[("localhost", "aa")]), &mut announced);

        let fresh = new_requests(
            &snapshot(&[("localhost", "aa"), ("localhost", "bb")]),
            &mut announced,
        );

        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].fingerprint, "bb");
    }
}
