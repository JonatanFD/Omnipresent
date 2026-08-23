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

use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

use omni_runtime::ipc::{
    Event, LayoutInfo, ModifierInfo, PeerInfo, Request, Response, StatusInfo, PROTOCOL_VERSION,
};
use omni_runtime::ipc_transport::connect_blocking;
use omni_runtime::Paths;
use tauri::{AppHandle, Emitter};

/// Pushed to the window whenever the daemon reports new state.
pub const STATUS_EVENT: &str = "daemon://status";
/// Pushed when the subscription drops, so the window can show it is out of touch.
pub const DISCONNECTED_EVENT: &str = "daemon://disconnected";

/// Sends one request and reads the single response line it gets back.
fn request(req: Request) -> Result<Response, String> {
    let paths = Paths::resolve().map_err(|e| e.to_string())?;
    let mut stream =
        connect_blocking(&paths).map_err(|_| "the daemon is not running".to_string())?;

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
pub fn daemon_stop() -> Result<(), String> {
    request(Request::Stop).map(|_| ())
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
    std::thread::spawn(move || loop {
        match subscribe_once(&app) {
            Ok(()) => {}
            Err(_) => {
                let _ = app.emit(DISCONNECTED_EVENT, ());
            }
        }
        // The daemon may still be starting, or may have been stopped on purpose.
        // Retrying on a slow beat costs nothing and recovers on its own.
        std::thread::sleep(Duration::from_secs(1));
    });
}

fn subscribe_once(app: &AppHandle) -> Result<(), String> {
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
            let _ = app.emit(STATUS_EVENT, status);
        }
    }
    Ok(())
}
