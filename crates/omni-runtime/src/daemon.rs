//! The daemon: the composition root that wires every adapter into the ports
//! and drives the pipelines.
//!
//! - **capture → route → send**: a dedicated thread polls the OS input source.
//!   Motion advances the virtual cursor through Topology; an edge crossing
//!   flips Session's active target, suppresses local input, and warps the
//!   peer's cursor to the entry point. Events bound for a remote peer go to
//!   that peer's task as QUIC datagrams.
//! - **receive → inject**: each established connection has a task that decodes
//!   incoming datagrams, validates the session, rate-limits, and injects.
//! - **IPC**: the local channel in the config dir — a Unix socket, or a named
//!   pipe on Windows — serves the `omni` CLI and the native GUIs.

use crate::config::{Config, Paths};
use crate::ipc::{
    Event, LayoutInfo, ModifierInfo, PROTOCOL_VERSION, PeerInfo, PendingInfo, Request, Response,
    SessionInfo, StatusInfo,
};
use crate::ipc_transport::{IpcListener, IpcStream};
use crate::ratelimit::RateLimiter;
use crate::trust::{TrustState, peer_id_of};
use omni_clipboard::adapter::ArboardAdapter;
use omni_clipboard::domain::ClipboardError;
use omni_clipboard::service::ClipboardManager;
use omni_input::platform::{OsSink, OsSource};
use omni_input::{InputSink, InputSource};
use omni_protocol::{
    ClipboardData, ControlMessage, Fingerprint, InputEvent, MachineId, Message, ModifierSwap,
    RejectReason, ScreenSize, SessionId,
};
use omni_session::{ActiveTarget, Role, SessionEvent, SessionEvents, SessionManager};
use omni_topology::{Crossing, CursorState, Edge, Machine, Point, Screen, VirtualLayout};
use omni_transport::{QuicConnection, QuicEndpoint, Transport, TransportError};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, watch};

/// How long the dialing side waits for the peer's user to accept.
const ACCEPT_TIMEOUT: Duration = Duration::from_secs(120);
/// How long an inbound connection may take to present its connect request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Capture poll interval when no events are waiting.
const CAPTURE_IDLE: Duration = Duration::from_micros(500);
/// How often each side sends a heartbeat on an established session.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
/// How long a session may go without any word from the peer before it is
/// treated as dead and torn down. Several heartbeats' worth, so a couple of
/// lost packets do not drop a healthy session.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(8);
/// How often the daemon checks the local clipboard for a new copy when sharing
/// is enabled. Fast enough to feel instant, slow enough to be free.
const CLIPBOARD_POLL: Duration = Duration::from_millis(500);
/// How long shutdown waits for tasks to finish before stopping anyway. Long
/// enough for orderly work to complete, short enough that the daemon always
/// exits rather than lingering with its socket held.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// Something a peer task is asked to do.
#[derive(Debug, PartialEq)]
enum PeerCommand {
    /// Forward one input event as an unreliable datagram.
    Input(InputEvent),
    /// Tell the peer to place its cursor at an absolute position (reliable).
    Warp { x: i32, y: i32 },
    /// Tell the peer the cursor has come back to this machine's screen, so its
    /// input belongs here now (reliable).
    CursorReturned { x: i32, y: i32 },
    /// Send a clipboard update to the peer over the reliable control stream.
    Clipboard(ClipboardData),
    /// End the session and close the connection.
    Disconnect,
}

/// One established peer connection, as the shared state sees it.
struct PeerLink {
    host: String,
    fingerprint: Fingerprint,
    session: SessionId,
    role: Role,
    screen: Screen,
    /// Which local edge this peer sits past.
    edge: Edge,
    /// How to relabel modifier keys on the way to this peer.
    modifier_swap: ModifierSwap,
    commands: mpsc::UnboundedSender<PeerCommand>,
}

/// An inbound connect request awaiting `omni accept` / `omni reject`.
struct PendingRequest {
    host: String,
    fingerprint: Fingerprint,
    decision: oneshot::Sender<bool>,
}

/// Session events go to the log; the CLI reads state via `omni status`.
struct LogEvents;

impl SessionEvents for LogEvents {
    fn emit(&mut self, event: SessionEvent) {
        tracing::info!(?event, "session event");
    }
}

/// Everything that changes while the daemon runs.
struct State {
    sessions: SessionManager<LogEvents>,
    layout: VirtualLayout,
    cursor: CursorState,
    links: HashMap<MachineId, PeerLink>,
    pending: Vec<PendingRequest>,
    /// Configured edge per peer host (from `omni layout`); overrides the
    /// default placement when that host connects.
    placements: HashMap<String, Edge>,
    /// Configured modifier relabelling per peer host (from `omni modifiers`).
    modifier_swaps: HashMap<String, ModifierSwap>,
}

/// Everything the tasks share.
struct Shared {
    state: Mutex<State>,
    trust: Arc<TrustState>,
    endpoint: QuicEndpoint,
    /// `None` when the OS denied injection access (target-only without source).
    sink: Option<Mutex<OsSink>>,
    /// `true` while local input is routed to a remote peer.
    suppress: watch::Sender<bool>,
    local_machine: MachineId,
    local_fingerprint: Fingerprint,
    local_screen: Screen,
    port: u16,
    /// The state directory, so layout changes can be persisted.
    paths: Paths,
    /// Whether the capture thread is alive (false = target-only).
    capturing: std::sync::atomic::AtomicBool,
    /// Opt-in clipboard sharing. Disabled by default; while off it neither reads
    /// the local clipboard nor applies a remote one.
    clipboard: ClipboardManager<ArboardAdapter>,
    /// Wakes or parks the clipboard polling task when sharing is toggled. The
    /// task idles for free while off (the default), so the opt-in costs nothing.
    clipboard_on: watch::Sender<bool>,
    /// Bumped whenever observable state changes (a session, a pending request,
    /// the active target, clipboard, or layout). Subscribers (`Request::Subscribe`)
    /// wake on the change and re-read a fresh `StatusInfo`. The value is just a
    /// counter; only "it changed" matters, and `watch` coalesces bursts.
    changes: watch::Sender<u64>,
    shutdown: tokio::sync::Notify,
    /// Flipped once the daemon is on its way down, so long-lived tasks can end
    /// on their own.
    ///
    /// A subscription would otherwise wait forever: the task blocks reading from
    /// a client that is itself waiting for the daemon to close the connection, so
    /// neither side moves and the process never finishes exiting. Any client that
    /// stays subscribed — which is exactly what the native GUIs do — used to leave
    /// `omni stop` reporting success while the daemon hung around still holding
    /// its socket.
    shutting_down: watch::Sender<bool>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("daemon state lock")
    }

    /// Re-derives the suppression flag from the active target.
    fn sync_suppression(&self, state: &State) {
        let suppress = matches!(state.sessions.active_target(), ActiveTarget::Remote(_));
        // Only signal real changes so the capture thread isn't woken idly.
        self.suppress.send_if_modified(|current| {
            if *current != suppress {
                *current = suppress;
                true
            } else {
                false
            }
        });
    }
}

/// Why the daemon failed to start.
#[derive(Debug)]
pub struct DaemonError(String);

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DaemonError {}

fn fail(context: &str, e: impl std::fmt::Display) -> DaemonError {
    DaemonError(format!("{context}: {e}"))
}

/// Runs the daemon in the foreground until `omni stop` or a signal. This is
/// what the hidden `omni daemon` subcommand calls. Uses the standard state
/// directory (`OMNI_CONFIG_DIR` or the platform default).
pub fn run() -> Result<(), DaemonError> {
    let paths = Paths::resolve().map_err(|e| fail("config", e))?;
    run_with_paths(paths)
}

/// Runs the daemon against an explicit state directory. Lets a test stand up
/// more than one daemon in a single process (where a shared `OMNI_CONFIG_DIR`
/// env var could not tell them apart).
pub fn run_with_paths(paths: Paths) -> Result<(), DaemonError> {
    // Before any screen query or input hook: make the process report and accept
    // real pixels. On a high-DPI display this is what keeps the captured deltas,
    // the parked cursor, and the virtual-desktop geometry in one coordinate
    // space (a no-op off Windows). Must run first to take effect.
    omni_input::platform::prepare_process();

    paths.ensure().map_err(|e| fail("config", e))?;
    init_logging(&paths);

    let config = Config::load(&paths).map_err(|e| fail("config", e))?;
    let identity = crate::identity::load_or_generate(&paths).map_err(|e| fail("identity", e))?;
    let trust = Arc::new(TrustState::load(paths.trust_file()).map_err(|e| fail("trust store", e))?);

    let local_fingerprint = identity.fingerprint();
    let local_machine = machine_id_of(local_fingerprint);
    let local_screen = detect_screen(&config);
    tracing::info!(
        fingerprint = %local_fingerprint,
        port = config.port(),
        screen = ?local_screen,
        "daemon starting"
    );

    let runtime = tokio::runtime::Runtime::new().map_err(|e| fail("tokio runtime", e))?;
    let result = runtime.block_on(async {
        let endpoint = QuicEndpoint::bind(
            SocketAddr::from(([0, 0, 0, 0], config.port())),
            &identity,
            trust.clone(),
        )
        .map_err(|e| fail("QUIC endpoint", e))?;

        let sink = match OsSink::new() {
            Ok(s) => Some(Mutex::new(s)),
            Err(e) => {
                tracing::warn!(%e, "input injection unavailable — running as relay only");
                None
            }
        };
        let (suppress_tx, suppress_rx) = watch::channel(false);
        let (clipboard_on_tx, clipboard_on_rx) = watch::channel(config.clipboard_sharing_enabled);
        let (changes_tx, _) = watch::channel(0u64);

        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                sessions: SessionManager::new(local_machine, LogEvents),
                layout: VirtualLayout::new(),
                cursor: CursorState::new(local_machine, Point::new(0, 0)),
                links: HashMap::new(),
                pending: Vec::new(),
                placements: config.placements.clone(),
                modifier_swaps: config.modifier_swaps.clone(),
            }),
            trust,
            endpoint,
            sink,
            suppress: suppress_tx,
            local_machine,
            local_fingerprint,
            local_screen,
            port: config.port(),
            paths: paths.clone(),
            capturing: std::sync::atomic::AtomicBool::new(false),
            clipboard: ClipboardManager::new(
                ArboardAdapter::new(),
                config.clipboard_sharing_enabled,
            ),
            clipboard_on: clipboard_on_tx,
            changes: changes_tx,
            shutdown: tokio::sync::Notify::new(),
            shutting_down: watch::channel(false).0,
        });
        rebuild_layout(&mut shared.lock(), &shared);

        // Capture: optional, so a machine that can only inject (e.g. no
        // accessibility yet) still works as a target.
        match OsSource::new() {
            Ok(source) => {
                shared
                    .capturing
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                let capture_shared = shared.clone();
                std::thread::Builder::new()
                    .name("omni-capture".into())
                    .spawn(move || run_capture(capture_shared, source, suppress_rx))
                    .map_err(|e| fail("capture thread", e))?;
            }
            Err(e) => {
                tracing::warn!(%e, "input capture unavailable — running as target only");
            }
        }

        // Clipboard sharing. The task parks while sharing is off (the default)
        // and wakes to poll only when it is on, so it can be toggled at runtime
        // (`omni clipboard on|off`) without restarting. While off the local
        // clipboard is never read.
        let clipboard_shared = shared.clone();
        tokio::spawn(async move { run_clipboard(clipboard_shared, clipboard_on_rx).await });

        // Inbound connections.
        let accept_shared = shared.clone();
        tokio::spawn(async move {
            while let Some(incoming) = accept_shared.endpoint.accept().await {
                match incoming {
                    Ok(connection) => {
                        let shared = accept_shared.clone();
                        tokio::spawn(handle_incoming(shared, connection));
                    }
                    Err(e) => tracing::debug!(%e, "inbound handshake failed"),
                }
            }
        });

        // IPC for the CLI: a Unix socket or a Windows named pipe, owner-scoped.
        //
        // Every task serving a client holds a clone of `alive`. None of them ever
        // sends on it; it exists so that when the last one ends, the receiver
        // reports the channel closed. That is how shutdown knows every client
        // connection has actually been let go — a client blocked reading needs
        // our end closed before it will ever return, and dropping the runtime
        // does not promise that.
        let mut listener = IpcListener::bind(&paths).map_err(|e| fail("IPC channel", e))?;
        let (alive, mut all_clients_done) = mpsc::channel::<()>(1);
        let ipc_shared = shared.clone();
        let accept_alive = alive.clone();
        let mut accept_shutdown = shared.shutting_down.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accepted = listener.accept() => match accepted {
                        Ok(stream) => {
                            let shared = ipc_shared.clone();
                            let alive = accept_alive.clone();
                            tokio::spawn(async move {
                                handle_client(shared, stream).await;
                                drop(alive);
                            });
                        }
                        Err(e) => {
                            tracing::warn!(%e, "IPC accept failed");
                            break;
                        }
                    },
                    // Stop accepting, and let go of this task's own token.
                    _ = accept_shutdown.changed() => break,
                }
            }
        });
        // The daemon's own token, dropped below so only the client tasks keep the
        // channel open.
        drop(alive);

        tracing::info!("daemon ready");
        wait_for_shutdown(&shared).await;

        tracing::info!("daemon shutting down");
        // Let the subscriptions go first: each is holding a connection open for a
        // client that will not close it until we do.
        let _ = shared.shutting_down.send(true);
        disconnect_all(&shared);
        shared.endpoint.close();
        // Wait for every client task to actually end, so their connections are
        // closed and anyone reading one gets end-of-stream rather than waiting
        // for a daemon that has already gone. Bounded, so a stuck client can
        // never keep the daemon alive.
        let _ = tokio::time::timeout(SHUTDOWN_GRACE, all_clients_done.recv()).await;
        Ok(())
    });

    // Dropping the runtime waits for the blocking pool, and clipboard reads live
    // there — the OS clipboard is a global lock any application may be holding,
    // so one can be slow or wedged through no fault of ours. Stopping with a
    // deadline means the daemon always exits: it gives tasks a moment to unwind
    // cleanly and then goes anyway.
    runtime.shutdown_timeout(SHUTDOWN_GRACE);
    result
}

fn init_logging(paths: &Paths) {
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.log_file())
    else {
        return;
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(file)
        .with_ansi(false)
        .try_init();
}

async fn wait_for_shutdown(shared: &Arc<Shared>) {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    tokio::select! {
        _ = shared.shutdown.notified() => {}
        _ = interrupt => {}
        _ = terminate_signal() => {}
    }
}

/// Resolves when the OS asks the daemon to terminate: `SIGTERM` on Unix, a
/// console-close event on Windows. Pends forever if the signal cannot be
/// registered, leaving `omni stop` and Ctrl-C as the ways out.
#[cfg(unix)]
async fn terminate_signal() {
    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
        Ok(mut sig) => {
            sig.recv().await;
        }
        Err(_) => std::future::pending().await,
    }
}

#[cfg(windows)]
async fn terminate_signal() {
    match tokio::signal::windows::ctrl_close() {
        Ok(mut sig) => {
            sig.recv().await;
        }
        Err(_) => std::future::pending().await,
    }
}

/// The machine identity everything keys on: derived from the certificate
/// fingerprint, so it is stable and unforgeable (proving it requires the key).
fn machine_id_of(fingerprint: Fingerprint) -> MachineId {
    MachineId::new(peer_id_of(fingerprint).value())
}

fn detect_screen(config: &Config) -> Screen {
    let (width, height) = omni_input::platform::primary_screen_size()
        .or(config.screen)
        .unwrap_or((1920, 1080));
    Screen::new(width, height)
}

/// Rebuilds the virtual desktop from the live links. Called whenever a
/// session comes or goes, so stale machines never linger.
fn rebuild_layout(state: &mut State, shared: &Shared) {
    let mut layout = VirtualLayout::new();
    let _ = layout.add_machine(Machine::new(shared.local_machine, shared.local_screen));
    for (machine, link) in &state.links {
        let _ = layout.add_machine(Machine::new(*machine, link.screen));
        let _ = layout.link(shared.local_machine, link.edge, *machine);
    }
    state.layout = layout;
    // If the cursor was tracked on a machine that vanished, bring it home.
    if state.layout.screen(state.cursor.machine).is_none() {
        state.cursor = CursorState::new(shared.local_machine, Point::new(0, 0));
    }
}

// ---------------------------------------------------------------------------
// Capture → route → send
// ---------------------------------------------------------------------------

fn run_capture(shared: Arc<Shared>, mut source: OsSource, suppress_rx: watch::Receiver<bool>) {
    let mut suppressed = false;
    loop {
        // Apply suppression decisions made anywhere (crossings, disconnects).
        let wanted = *suppress_rx.borrow();
        if wanted != suppressed {
            source.set_suppressed(wanted);
            suppressed = wanted;
        }

        match source.poll() {
            Ok(Some(event)) => route_captured(&shared, event),
            Ok(None) => std::thread::sleep(CAPTURE_IDLE),
            Err(e) => {
                shared
                    .capturing
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                tracing::error!(%e, "input capture stopped");
                return;
            }
        }
    }
}

/// One captured event: track the cursor, detect crossings, forward to the
/// active remote peer.
fn route_captured(shared: &Arc<Shared>, event: InputEvent) {
    let mut crossed = false;
    {
        let mut state = shared.lock();
        match state.sessions.active_target() {
            ActiveTarget::Local => {
                if let InputEvent::Motion(delta) = event {
                    sync_cursor_to_os(&mut state, shared);
                    crossed = advance_cursor(&mut state, shared, delta);
                }
                // Non-motion local input is none of our business.
            }
            ActiveTarget::Remote(peer) => match event {
                InputEvent::Motion(delta) => {
                    crossed = advance_cursor(&mut state, shared, delta);
                    if !crossed {
                        // Still on the remote screen: send the cursor's absolute
                        // position on that screen, not the raw delta. The
                        // virtual desktop already mapped it into the peer's
                        // pixels using both machines' sizes, so the peer's cursor
                        // lands exactly here with no drift.
                        let point = state.cursor.position;
                        forward_to(
                            &state,
                            peer,
                            InputEvent::Pointer {
                                x: point.x as i32,
                                y: point.y as i32,
                            },
                        );
                    }
                }
                other => forward_to(&state, peer, other),
            },
        }
        shared.sync_suppression(&state);
    }
    // A crossing flips which machine has input — surface that to subscribers.
    if crossed {
        notify_change(shared);
    }
}

/// While input is local the OS owns the real cursor (acceleration and all);
/// adopt its position so edge detection matches what the user sees.
fn sync_cursor_to_os(state: &mut State, shared: &Shared) {
    if let Some((x, y)) = omni_input::platform::cursor_position() {
        let clamped = clamp_to_screen(shared.local_screen, x, y);
        state.cursor = CursorState::new(shared.local_machine, clamped);
    }
}

/// A position kept inside `screen`. Where the cursor is tracked always has to be
/// a real spot on the screen holding it: a position outside would read as a
/// crossing the moment the mouse next moves.
fn clamp_to_screen(screen: Screen, x: i32, y: i32) -> Point {
    Point::new(
        (x.max(0) as u32).min(screen.width.saturating_sub(1)),
        (y.max(0) as u32).min(screen.height.saturating_sub(1)),
    )
}

/// Moves the virtual cursor. Returns `true` when the move crossed an edge
/// (and the crossing was handled), `false` when it stayed on screen.
fn advance_cursor(state: &mut State, shared: &Shared, delta: omni_protocol::MouseDelta) -> bool {
    // The movement was measured on this machine's screen, in whatever unit this
    // OS reports. While the cursor is on a peer it has to move by the same
    // *fraction* of that peer's screen, or it visibly changes speed at the
    // crossing — a Retina Mac and a 4K PC describe their screens in units that
    // are nowhere near the same size.
    let delta = match state.layout.screen(state.cursor.machine) {
        Some(screen) if state.cursor.machine != shared.local_machine => {
            omni_topology::scale_delta(delta, shared.local_screen, screen)
        }
        _ => delta,
    };
    let advance = match state.layout.advance(state.cursor, delta) {
        Ok(advance) => advance,
        Err(e) => {
            tracing::warn!(%e, "cursor tracking lost; resetting to local");
            state.cursor = CursorState::new(shared.local_machine, Point::new(0, 0));
            return false;
        }
    };
    let Some(crossing) = advance.crossing else {
        state.cursor = advance.cursor;
        return false;
    };
    if state.sessions.handle_crossing(crossing).is_err() {
        // An edge to a machine we have no session with: stay put.
        return false;
    }
    state.cursor = advance.cursor;
    place_cursor_after_crossing(state, shared, crossing);
    true
}

/// Puts the real cursor where the virtual one just landed: on the peer via a
/// reliable warp message, or locally via the sink.
///
/// Either way the peers are told, because a crossing is also what decides where
/// every keyboard types. The cursor landing on a peer is that peer's cue to take
/// its input back ([`ControlMessage::CursorWarp`]); the cursor landing back here
/// is every peer's cue to send its input to this machine
/// ([`ControlMessage::CursorReturned`]). Only the first half used to travel, so
/// a peer never learned that the cursor had left it and went on typing on its
/// own desktop.
fn place_cursor_after_crossing(state: &State, shared: &Shared, crossing: Crossing) {
    let x = crossing.entry.x as i32;
    let y = crossing.entry.y as i32;
    if crossing.peer == shared.local_machine {
        if let Some(ref sm) = shared.sink
            && let Ok(mut sink) = sm.lock()
            && let Err(e) = sink.warp(x, y)
        {
            tracing::warn!(%e, "could not place the local cursor");
        }
        for link in state.links.values() {
            let _ = link.commands.send(PeerCommand::CursorReturned { x, y });
        }
    } else if let Some(link) = state.links.get(&crossing.peer) {
        let _ = link.commands.send(PeerCommand::Warp { x, y });
    }
}

fn forward_to(state: &State, peer: MachineId, event: InputEvent) {
    if let Some(link) = state.links.get(&peer) {
        // Relabel modifiers here, on the way out: the controller is the side that
        // knows which peer this is going to, so the target can inject whatever it
        // receives without knowing anything about us.
        let event = link.modifier_swap.apply(event);
        let _ = link.commands.send(PeerCommand::Input(event));
    }
}

// ---------------------------------------------------------------------------
// Clipboard sharing (opt-in)
// ---------------------------------------------------------------------------

/// Polls the local clipboard and broadcasts any new copy to every connected
/// peer over their reliable control streams. It parks for free while sharing is
/// off and polls only while it is on, so the opt-in default costs nothing and
/// the toggle takes effect at once. Ends when the daemon shuts down.
async fn run_clipboard(shared: Arc<Shared>, mut enabled: watch::Receiver<bool>) {
    let mut shutting_down = shared.shutting_down.subscribe();
    loop {
        // Park until sharing is turned on. No clipboard read happens here.
        while !*enabled.borrow() {
            tokio::select! {
                changed = enabled.changed() => {
                    if changed.is_err() {
                        return; // daemon shutting down
                    }
                }
                _ = shutting_down.changed() => return,
            }
        }
        // Sharing is on: poll until it is turned back off.
        let mut interval = tokio::time::interval(CLIPBOARD_POLL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = interval.tick() => broadcast_local_clipboard(&shared).await,
                changed = enabled.changed() => {
                    if changed.is_err() {
                        return; // daemon shutting down
                    }
                    if !*enabled.borrow() {
                        break; // back to parking
                    }
                }
                // Stop before the next poll: reading the clipboard blocks, and a
                // shutdown should not have to wait behind one.
                _ = shutting_down.changed() => return,
            }
        }
    }
}

/// Reads the local clipboard and, on a genuinely new copy, sends it to every
/// connected peer — not just the active target, so it is available wherever the
/// user pastes. The manager's echo guard stops it bouncing back.
///
/// Reading the OS clipboard *blocks*, and on Windows it can block for a while:
/// the clipboard is a single global lock that any application may be holding.
/// Doing that directly on an async worker stalls every other task sharing the
/// thread, and stalls shutdown behind it. It goes to the blocking pool instead.
async fn broadcast_local_clipboard(shared: &Arc<Shared>) {
    let reader = shared.clone();
    let read = tokio::task::spawn_blocking(move || reader.clipboard.poll_local_change()).await;
    let data = match read {
        Ok(Ok(Some(data))) => data,
        Ok(Ok(None)) | Ok(Err(ClipboardError::Disabled)) => return,
        Ok(Err(e)) => {
            tracing::warn!(%e, "could not read the local clipboard");
            return;
        }
        // The blocking pool is going away with the runtime; nothing to report.
        Err(_) => return,
    };
    let state = shared.lock();
    for link in state.links.values() {
        let _ = link.commands.send(PeerCommand::Clipboard(data.clone()));
    }
}

// ---------------------------------------------------------------------------
// Connections (both directions) and the per-peer task
// ---------------------------------------------------------------------------

/// An inbound connection: read the connect request, decide (auto-trust or ask
/// the user), reply, and run the session.
async fn handle_incoming(shared: Arc<Shared>, connection: QuicConnection) {
    let fingerprint = connection.peer_fingerprint();
    let host = connection.remote_address().ip().to_string();

    let mut control = match connection.accept_control().await {
        Ok(control) => control,
        Err(e) => {
            tracing::debug!(%host, %e, "no control stream from peer");
            return;
        }
    };
    let request = tokio::time::timeout(REQUEST_TIMEOUT, control.recv()).await;
    let screen = match request {
        Ok(Ok(Some(Message::Control(ControlMessage::ConnectRequest { screen, .. })))) => screen,
        _ => {
            tracing::debug!(%host, "peer sent no valid connect request");
            connection.close();
            return;
        }
    };

    let trusted = shared.trust.is_trusted(fingerprint);
    if !trusted {
        tracing::info!(
            %host,
            %fingerprint,
            "incoming connection request — approve with `omni accept {host}`"
        );
        let (decision_tx, decision_rx) = oneshot::channel();
        shared.lock().pending.push(PendingRequest {
            host: host.clone(),
            fingerprint,
            decision: decision_tx,
        });
        notify_change(&shared);
        let approved = matches!(
            tokio::time::timeout(ACCEPT_TIMEOUT, decision_rx).await,
            Ok(Ok(true))
        );
        // Whatever happened, this request is no longer pending.
        shared
            .lock()
            .pending
            .retain(|p| p.fingerprint != fingerprint);
        notify_change(&shared);
        if !approved {
            let _ = control
                .send(&Message::Control(ControlMessage::Reject {
                    reason: RejectReason::Declined,
                }))
                .await;
            connection.close();
            return;
        }
        if let Err(e) = shared.trust.accept(fingerprint, Some(&host)) {
            tracing::error!(%e, "could not persist trust");
        }
    }

    // Trusted (pinned earlier or just approved): establish the session.
    let machine = machine_id_of(fingerprint);
    let session = SessionId::new(rand::random::<u128>());
    let (commands_tx, commands_rx) = mpsc::unbounded_channel();
    let established = {
        let mut state = shared.lock();
        match state.sessions.establish(session, machine, Role::Target) {
            Ok(()) => {
                // The controller reached us, so by default it sits past our
                // left edge — unless `omni layout` placed this host elsewhere.
                let edge = state.placements.get(&host).copied().unwrap_or(Edge::Left);
                let modifier_swap = state.modifier_swaps.get(&host).copied().unwrap_or_default();
                state.links.insert(
                    machine,
                    PeerLink {
                        host: host.clone(),
                        fingerprint,
                        session,
                        role: Role::Target,
                        screen: Screen::new(screen.width, screen.height),
                        edge,
                        modifier_swap,
                        commands: commands_tx,
                    },
                );
                rebuild_layout(&mut state, &shared);
                true
            }
            Err(e) => {
                tracing::warn!(%host, %e, "cannot establish session");
                false
            }
        }
    };
    if !established {
        let _ = control
            .send(&Message::Control(ControlMessage::Reject {
                reason: RejectReason::Busy,
            }))
            .await;
        connection.close();
        return;
    }
    notify_change(&shared);

    let accept = Message::Control(ControlMessage::Accept {
        session,
        machine: shared.local_machine,
        screen: ScreenSize::new(shared.local_screen.width, shared.local_screen.height),
    });
    if let Err(e) = control.send(&accept).await {
        tracing::warn!(%host, %e, "could not send accept");
        cleanup_peer(&shared, machine);
        connection.close();
        return;
    }

    tracing::info!(%host, %fingerprint, "session established (target)");
    run_peer(shared, connection, control, commands_rx, session, machine).await;
}

/// An outbound connection (from `omni connect`).
async fn do_connect(shared: &Arc<Shared>, host_arg: &str) -> Result<(), String> {
    let (host, addr) = resolve_host(host_arg, shared.port).await?;
    let connection = shared
        .endpoint
        .connect(addr, &host)
        .await
        .map_err(|e| connect_error(host_arg, &host, e))?;
    let fingerprint = connection.peer_fingerprint();

    let mut control = connection
        .open_control()
        .await
        .map_err(|e| format!("control stream failed: {e}"))?;
    control
        .send(&Message::Control(ControlMessage::ConnectRequest {
            machine: shared.local_machine,
            fingerprint: shared.local_fingerprint,
            screen: ScreenSize::new(shared.local_screen.width, shared.local_screen.height),
        }))
        .await
        .map_err(|e| format!("could not send connect request: {e}"))?;

    let reply = tokio::time::timeout(ACCEPT_TIMEOUT, control.recv())
        .await
        .map_err(|_| "timed out waiting for the peer to accept".to_string())
        .and_then(|r| r.map_err(|e| format!("connection failed while waiting: {e}")))?;

    let (session, screen) = match reply {
        Some(Message::Control(ControlMessage::Accept {
            session, screen, ..
        })) => (session, screen),
        Some(Message::Control(ControlMessage::Reject { reason })) => {
            connection.close();
            return Err(match reason {
                RejectReason::Declined => "the peer declined the request".into(),
                RejectReason::Busy => "the peer is busy with another session".into(),
                RejectReason::NotAllowed => "the peer does not allow this machine".into(),
                RejectReason::FingerprintChanged => {
                    "the peer rejected this machine's certificate (fingerprint changed)".into()
                }
            });
        }
        _ => {
            connection.close();
            return Err("the peer closed the connection without answering".into());
        }
    };

    // TOFU: dialing was the intent, the peer accepted — pin its fingerprint
    // for this host. A later certificate change will refuse the handshake.
    shared
        .trust
        .accept(fingerprint, Some(&host))
        .map_err(|e| format!("could not persist trust: {e}"))?;

    let machine = machine_id_of(fingerprint);
    let (commands_tx, commands_rx) = mpsc::unbounded_channel();
    {
        let mut state = shared.lock();
        state
            .sessions
            .establish(session, machine, Role::Controller)
            .map_err(|e| format!("cannot establish session: {e}"))?;
        // We dialed it: it sits past our right edge unless `omni layout` placed
        // this host somewhere else.
        let edge = state.placements.get(&host).copied().unwrap_or(Edge::Right);
        let modifier_swap = state.modifier_swaps.get(&host).copied().unwrap_or_default();
        state.links.insert(
            machine,
            PeerLink {
                host: host.clone(),
                fingerprint,
                session,
                role: Role::Controller,
                screen: Screen::new(screen.width, screen.height),
                edge,
                modifier_swap,
                commands: commands_tx,
            },
        );
        rebuild_layout(&mut state, shared);
    }
    notify_change(shared);

    tracing::info!(%host, %fingerprint, "session established (controller)");
    let task_shared = shared.clone();
    tokio::spawn(async move {
        run_peer(
            task_shared,
            connection,
            control,
            commands_rx,
            session,
            machine,
        )
        .await;
    });
    Ok(())
}

/// Turns a failed dial into a user-facing message, adding a recovery hint when
/// the failure is the peer's certificate being refused — almost always because
/// that host legitimately got a new certificate (reinstall, fresh config) and
/// the old one is still pinned here under TOFU.
fn connect_error(host_arg: &str, host: &str, e: omni_transport::QuicError) -> String {
    let detail = e.to_string();
    let looks_like_tofu = detail.contains("certificate") || detail.contains("handshake");
    if looks_like_tofu {
        format!(
            "could not connect to {host_arg}: {detail}\n\
             hint: if {host} was reinstalled or its identity reset, its certificate \
             changed; run `omni peers remove {host}` here, then connect again to trust \
             the new one"
        )
    } else {
        format!("could not connect to {host_arg}: {detail}")
    }
}

/// `host[:port]` → (host-for-TOFU, socket address). A bare IPv6 literal
/// (`::1`) is taken whole; only a single-colon form is split as host:port.
async fn resolve_host(host_arg: &str, default_port: u16) -> Result<(String, SocketAddr), String> {
    let (host, port) = match host_arg.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') && p.parse::<u16>().is_ok() => {
            (h.to_string(), p.parse::<u16>().unwrap())
        }
        _ => (host_arg.to_string(), default_port),
    };
    let addr = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|e| format!("could not resolve {host}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address for {host}"))?;
    Ok((host, addr))
}

/// Sends clipboard payloads to one peer, one at a time and in the order they
/// were copied.
///
/// This runs as its own task so that writing a large payload — a screenshot is
/// megabytes — never happens on the task that also has to send and answer
/// heartbeats. It used to, and a copy big enough to take a few seconds looked
/// exactly like a machine that had stopped responding, so both ends tore the
/// session down.
///
/// Sequential on purpose: a payload finishes before the next one starts, so a
/// newer copy cannot overtake an older one and leave the peer with the wrong
/// clipboard. `send` reports whether the payload went out; once it cannot, the
/// connection is gone and there is nothing left to send.
async fn run_clipboard_sender<S, F>(mut payloads: mpsc::UnboundedReceiver<ClipboardData>, send: S)
where
    S: Fn(ClipboardData) -> F,
    F: std::future::Future<Output = bool>,
{
    while let Some(data) = payloads.recv().await {
        if !send(data).await {
            break;
        }
    }
}

/// The per-connection task: pump datagrams in, commands out, until the
/// session or the connection ends.
async fn run_peer(
    shared: Arc<Shared>,
    mut connection: QuicConnection,
    control: omni_transport::ControlStream,
    mut commands: mpsc::UnboundedReceiver<PeerCommand>,
    session: SessionId,
    machine: MachineId,
) {
    let (mut control_tx, mut control_rx) = control.split();

    // Clipboard payloads leave on their own QUIC stream, written by their own
    // task. Nothing below may await a large write: this loop is what keeps the
    // session alive, and a peer that stops answering for a few seconds is
    // indistinguishable from a peer that has died.
    let bulk = connection.bulk_sender();
    let mut incoming_bulk = connection
        .take_bulk_receiver()
        .expect("a fresh connection still has its bulk receiver");
    let (clipboard_tx, clipboard_rx) = mpsc::unbounded_channel::<ClipboardData>();
    tokio::spawn(run_clipboard_sender(clipboard_rx, move |data| {
        let bulk = bulk.clone();
        async move {
            match bulk.send(&Message::Clipboard(data)).await {
                Ok(()) => true,
                Err(e) => {
                    tracing::debug!(%e, "could not send the clipboard to the peer");
                    false
                }
            }
        }
    }));

    let mut transport = Transport::new(connection);
    let mut limiter = RateLimiter::default();
    let mut dropped: u64 = 0;

    // Heartbeats: send one every interval, and treat the peer as dead if
    // nothing has arrived from it within the timeout. Any inbound traffic
    // (input datagram or control message) counts as proof of life.
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut deadline = tokio::time::Instant::now() + HEARTBEAT_TIMEOUT;

    loop {
        let liveness = tokio::time::sleep_until(deadline);
        tokio::select! {
            command = commands.recv() => {
                let Some(first) = command else {
                    // The channel closed: the session is going away.
                    let goodbye = Message::Control(ControlMessage::Disconnect { session });
                    let _ = control_tx.send(&goodbye).await;
                    break;
                };
                // Drain everything queued right now and collapse runs of cursor
                // motion into the latest position. Under congestion the channel
                // backs up with stale absolute positions; sending them all would
                // only make the cursor lag behind the real one.
                let mut batch = vec![first];
                while let Ok(next) = commands.try_recv() {
                    batch.push(next);
                }
                let mut closing = false;
                for command in coalesce_motion(batch) {
                    match command {
                        PeerCommand::Input(event) => {
                            let message = Message::Input { session, event };
                            if let Err(TransportError::Channel(e)) = transport.send(&message) {
                                tracing::debug!(%e, "datagram send failed");
                            }
                        }
                        PeerCommand::Warp { x, y } => {
                            let message =
                                Message::Control(ControlMessage::CursorWarp { session, x, y });
                            if control_tx.send(&message).await.is_err() {
                                closing = true;
                                break;
                            }
                        }
                        PeerCommand::CursorReturned { x, y } => {
                            let message =
                                Message::Control(ControlMessage::CursorReturned { session, x, y });
                            if control_tx.send(&message).await.is_err() {
                                closing = true;
                                break;
                            }
                        }
                        PeerCommand::Clipboard(data) => {
                            // Handed over, not written here: see
                            // `run_clipboard_sender`. This must not wait.
                            if clipboard_tx.send(data).is_err() {
                                closing = true;
                                break;
                            }
                        }
                        PeerCommand::Disconnect => {
                            let goodbye =
                                Message::Control(ControlMessage::Disconnect { session });
                            let _ = control_tx.send(&goodbye).await;
                            closing = true;
                            break;
                        }
                    }
                }
                if closing {
                    break;
                }
            },
            incoming = transport.recv_async() => {
                deadline = tokio::time::Instant::now() + HEARTBEAT_TIMEOUT;
                match incoming {
                    Ok(Some(Message::Input { session: claimed, event })) => {
                        if claimed != session {
                            continue; // not part of this session: drop silently
                        }
                        if !limiter.allow() {
                            dropped += 1;
                            if dropped.is_multiple_of(1_000) {
                                tracing::warn!(dropped, "rate limit: dropping input events");
                            }
                            continue;
                        }
                        inject(&shared, event);
                    }
                    Ok(Some(_)) => {} // control messages do not ride datagrams
                    Ok(None) => break, // connection closed
                    Err(TransportError::Codec(e)) => {
                        tracing::debug!(%e, "undecodable datagram dropped");
                    }
                    Err(TransportError::Channel(_)) => break,
                }
            },
            signalling = control_rx.recv() => {
                deadline = tokio::time::Instant::now() + HEARTBEAT_TIMEOUT;
                match signalling {
                    Ok(Some(Message::Control(ControlMessage::Disconnect { .. }))) | Ok(None) => break,
                    Ok(Some(Message::Control(ControlMessage::CursorWarp { session: claimed, x, y }))) => {
                        if claimed == session {
                            // The peer's cursor has just crossed onto this
                            // machine, so it is the one in control now.
                            yield_control_to_peer(&shared);
                            warp(&shared, x, y);
                        }
                    }
                    Ok(Some(Message::Control(ControlMessage::CursorReturned { session: claimed, x, y }))) => {
                        if claimed == session {
                            // The cursor has left this machine for the peer's own
                            // screen, so this machine's input follows it there.
                            follow_peer_cursor(&shared, machine, x, y);
                        }
                    }
                    Ok(Some(_)) => {} // heartbeats keep the session alive
                    Err(_) => break,
                }
            },
            payload = incoming_bulk.recv() => {
                // Clipboard payloads arrive on their own stream, already read
                // whole by the transport's pump — so a large one never held
                // this loop up while it was in transit.
                match payload {
                    Some(Message::Clipboard(data)) => {
                        deadline = tokio::time::Instant::now() + HEARTBEAT_TIMEOUT;
                        // Applied locally, and silently ignored when sharing is
                        // off (the manager returns `Disabled`).
                        match shared.clipboard.handle_remote_update(data) {
                            Ok(()) | Err(ClipboardError::Disabled) => {}
                            Err(e) => {
                                tracing::warn!(%e, "could not apply remote clipboard update");
                            }
                        }
                    }
                    Some(_) => {} // only clipboard payloads travel in bulk
                    None => break, // connection closed
                }
            },
            _ = heartbeat.tick() => {
                let beat = Message::Control(ControlMessage::Heartbeat { session });
                if control_tx.send(&beat).await.is_err() {
                    break;
                }
            }
            _ = liveness => {
                tracing::info!(machine = machine.value(), "peer timed out — no heartbeat");
                break;
            }
        }
    }

    transport.channel().close();
    cleanup_peer(&shared, machine);
    tracing::info!(machine = machine.value(), "session closed");
}

/// Collapses runs of consecutive cursor-position updates into just the most
/// recent one. The cursor position is absolute, so once a newer position
/// exists every older one in the backlog is stale and worth dropping — sending
/// them would only make the remote cursor trail behind. Keys, clicks, and
/// scrolls are order-sensitive (a click must land where it was made), so they
/// are never dropped and they break a run of pointer updates.
fn coalesce_motion(batch: Vec<PeerCommand>) -> Vec<PeerCommand> {
    let mut out: Vec<PeerCommand> = Vec::with_capacity(batch.len());
    for command in batch {
        let is_pointer = matches!(command, PeerCommand::Input(InputEvent::Pointer { .. }));
        let prev_is_pointer = matches!(
            out.last(),
            Some(PeerCommand::Input(InputEvent::Pointer { .. }))
        );
        if is_pointer && prev_is_pointer {
            // Replace the stale position with this fresher one.
            *out.last_mut().expect("checked non-empty above") = command;
        } else {
            out.push(command);
        }
    }
    out
}

fn inject(shared: &Shared, event: InputEvent) {
    if let Some(ref sm) = shared.sink
        && let Ok(mut sink) = sm.lock()
        && let Err(e) = sink.inject(event)
    {
        tracing::warn!(%e, "could not inject input");
    }
}

/// Hands control of this machine to the peer that just crossed onto it: input
/// goes back to the local screen and the local keyboard and mouse are released.
///
/// Either machine's user can push their cursor across at any moment, and each
/// one decides that for itself. If both did so, each would send its input to the
/// other and withhold it from its own desktop, so neither would act on what it
/// received — which looked like a machine that could be moved around but not
/// clicked or typed on. Giving way to whoever crossed last settles it: the user
/// who reached for their mouse is the one driving.
fn yield_control_to_peer(shared: &Arc<Shared>) {
    let gave_way = {
        let mut state = shared.lock();
        let gave_way = state.sessions.yield_control();
        if gave_way {
            // The cursor is wherever the peer is about to put it; adopt that on
            // the next local movement rather than the stale tracked position.
            state.cursor = CursorState::new(shared.local_machine, Point::new(0, 0));
            shared.sync_suppression(&state);
        }
        gave_way
    };
    if gave_way {
        tracing::info!("a peer took control; input is back on the local screen");
        notify_change(shared);
    }
}

/// Sends this machine's input to the peer, which has just taken the cursor back
/// onto its own screen at `(x, y)`.
///
/// The mirror of [`yield_control_to_peer`], and the half that used to be
/// missing. Giving way when a peer crosses onto this machine is only one side of
/// the hand-over: when the cursor leaves again this machine has to hear about it
/// too, or its keyboard goes on typing on its own desktop while the cursor sits
/// on the peer — which is what made a keyboard work locally and nowhere else.
/// The position comes along so the one shared cursor keeps being tracked from
/// where it really is, rather than from wherever this machine last saw it.
fn follow_peer_cursor(shared: &Arc<Shared>, peer: MachineId, x: i32, y: i32) {
    let followed = {
        let mut state = shared.lock();
        // Without that peer's screen on record there is nothing to track the
        // cursor against, and the next movement would only lose it again.
        let Some(screen) = state.layout.screen(peer) else {
            return;
        };
        let before = state.sessions.active_target();
        if let Err(e) = state.sessions.follow_peer(peer) {
            tracing::debug!(%e, "ignoring a hand-over from a peer with no session");
            return;
        }
        // Refreshed even when this machine was already following that peer: the
        // message says where the cursor really is, which is worth more than
        // wherever this machine last worked it out to be.
        state.cursor = CursorState::new(peer, clamp_to_screen(screen, x, y));
        shared.sync_suppression(&state);
        before != ActiveTarget::Remote(peer)
    };
    if followed {
        tracing::info!(
            machine = peer.value(),
            "the cursor is on a peer; local input follows it there"
        );
        notify_change(shared);
    }
}

fn warp(shared: &Shared, x: i32, y: i32) {
    if let Some(ref sm) = shared.sink
        && let Ok(mut sink) = sm.lock()
        && let Err(e) = sink.warp(x, y)
    {
        tracing::warn!(%e, "could not warp cursor");
    }
}

fn cleanup_peer(shared: &Arc<Shared>, machine: MachineId) {
    {
        let mut state = shared.lock();
        if let Some(link) = state.links.remove(&machine) {
            let _ = state.sessions.close(link.session);
        }
        rebuild_layout(&mut state, shared);
        shared.sync_suppression(&state);
    }
    notify_change(shared);
}

/// Signals subscribers that observable state changed, so a `Request::Subscribe`
/// connection re-reads and pushes a fresh status. Safe to call whether or not the
/// caller holds the state lock — it only bumps a counter, never locks state.
fn notify_change(shared: &Shared) {
    shared.changes.send_modify(|n| *n = n.wrapping_add(1));
}

fn disconnect_all(shared: &Arc<Shared>) {
    let state = shared.lock();
    for link in state.links.values() {
        let _ = link.commands.send(PeerCommand::Disconnect);
    }
}

// ---------------------------------------------------------------------------
// IPC
// ---------------------------------------------------------------------------

async fn handle_client(shared: Arc<Shared>, stream: IpcStream) {
    let (read, mut write) = tokio::io::split(stream);
    let mut lines = BufReader::new(read).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let request = serde_json::from_str::<Request>(&line);
        // A subscription turns this connection into a one-way event stream until
        // the client disconnects, so it never returns to the request loop.
        if matches!(request, Ok(Request::Subscribe)) {
            stream_events(&shared, &mut write, &mut lines).await;
            return;
        }
        let stopping = matches!(request, Ok(Request::Stop));
        let response = match request {
            Ok(request) => dispatch(&shared, request).await,
            Err(e) => Response::Error {
                message: format!("bad request: {e}"),
            },
        };
        let Ok(mut payload) = serde_json::to_vec(&response) else {
            break;
        };
        payload.push(b'\n');
        if write.write_all(&payload).await.is_err() {
            break;
        }
        if stopping {
            // The reply is flushed first so `omni stop` sees its answer.
            let _ = write.flush().await;
            shared.shutdown.notify_one();
        }
    }
}

/// Drives a `Request::Subscribe` connection: pushes an initial status snapshot,
/// then a fresh one every time state changes, until the client disconnects.
/// Reads are only used to detect that disconnect — a subscription is push-only.
async fn stream_events<W, R>(shared: &Arc<Shared>, write: &mut W, lines: &mut tokio::io::Lines<R>)
where
    W: tokio::io::AsyncWrite + Unpin,
    R: tokio::io::AsyncBufRead + Unpin,
{
    let mut changes = shared.changes.subscribe();
    let mut shutting_down = shared.shutting_down.subscribe();
    // Subscribing only arms us for *later* changes, so a shutdown that already
    // began is invisible to `changed()`. Without this check a subscription that
    // arrives in that window waits on a client that is waiting on us, and the
    // daemon never finishes exiting — the same deadlock the signal exists to
    // prevent, just through a narrower door.
    if *shutting_down.borrow() {
        return;
    }
    if !write_event(write, &Event::Status(status(shared))).await {
        return;
    }
    loop {
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    break; // the daemon is shutting down
                }
                if !write_event(write, &Event::Status(status(shared))).await {
                    break; // client gone
                }
            }
            _ = shutting_down.changed() => break,
            line = lines.next_line() => match line {
                // Stray input on a subscription is ignored; EOF/error means the
                // client disconnected.
                Ok(Some(_)) => {}
                _ => break,
            },
        }
    }
}

/// Writes one event as a JSON line. Returns `false` if the write failed (the
/// client is gone), so the caller can stop.
async fn write_event<W>(write: &mut W, event: &Event) -> bool
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let Ok(mut payload) = serde_json::to_vec(event) else {
        return false;
    };
    payload.push(b'\n');
    write.write_all(&payload).await.is_ok()
}

async fn dispatch(shared: &Arc<Shared>, request: Request) -> Response {
    match request {
        Request::Hello => Response::Hello {
            protocol_version: PROTOCOL_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        // Subscribe is handled at the connection level (it streams, not a single
        // reply); this arm only keeps the match exhaustive and is never reached.
        Request::Subscribe => Response::Error {
            message: "subscribe is handled at the connection level".into(),
        },
        Request::Status => Response::Status(status(shared)),
        Request::Stop => Response::Ok,
        Request::Connect { host } => match do_connect(shared, &host).await {
            Ok(()) => Response::Ok,
            Err(message) => Response::Error { message },
        },
        Request::Disconnect { host } => {
            let state = shared.lock();
            let link = state
                .links
                .values()
                .find(|l| l.host == host || l.fingerprint.to_string().starts_with(&host));
            match link {
                Some(link) => {
                    let _ = link.commands.send(PeerCommand::Disconnect);
                    Response::Ok
                }
                None => Response::Error {
                    message: format!("no active session with {host}"),
                },
            }
        }
        Request::Accept { selector } => decide_pending(shared, &selector, true),
        Request::Reject { selector } => decide_pending(shared, &selector, false),
        Request::Peers => Response::Peers {
            peers: list_peers(shared),
        },
        Request::RemovePeer { selector } => {
            {
                // Drop any live session with the peer being removed.
                let state = shared.lock();
                if let Some(link) = state.links.values().find(|l| {
                    l.host == selector || l.fingerprint.to_string().starts_with(&selector)
                }) {
                    let _ = link.commands.send(PeerCommand::Disconnect);
                }
            }
            match shared.trust.remove(&selector) {
                Ok(true) => Response::Ok,
                Ok(false) => Response::Error {
                    message: format!("no known peer matches {selector}"),
                },
                Err(e) => Response::Error {
                    message: format!("could not update the trust store: {e}"),
                },
            }
        }
        Request::Layout { host, edge } => match (host, edge) {
            (Some(host), Some(edge)) => set_layout(shared, &host, &edge),
            (None, None) => Response::Layout {
                placements: list_layout(shared),
            },
            _ => Response::Error {
                message: "give both a host and an edge to place a peer, or \
                          neither to list placements"
                    .into(),
            },
        },
        Request::Clipboard { enabled } => set_clipboard(shared, enabled),
        Request::Modifiers { host, swap } => match (host, swap) {
            (Some(host), Some(swap)) => set_modifier_swap(shared, &host, &swap),
            (None, None) => Response::Modifiers {
                swaps: list_modifier_swaps(shared),
            },
            _ => Response::Error {
                message: "give both a host and a swap to set one, or neither to \
                          list them"
                    .into(),
            },
        },
    }
}

/// Sets how modifier keys are relabelled for a peer host: records it, persists
/// it, and applies it to any live session with that host straight away.
fn set_modifier_swap(shared: &Arc<Shared>, host: &str, swap: &str) -> Response {
    let Some(swap) = ModifierSwap::parse(swap) else {
        return Response::Error {
            message: format!("unknown swap '{swap}' — use none or meta-control"),
        };
    };

    {
        let mut state = shared.lock();
        state.modifier_swaps.insert(host.to_string(), swap);
        let live: Vec<MachineId> = state
            .links
            .iter()
            .filter(|(_, link)| link.host == host)
            .map(|(machine, _)| *machine)
            .collect();
        for machine in live {
            if let Some(link) = state.links.get_mut(&machine) {
                link.modifier_swap = swap;
            }
        }
    }
    notify_change(shared);

    match Config::load(&shared.paths) {
        Ok(mut config) => {
            config.modifier_swaps.insert(host.to_string(), swap);
            if let Err(e) = config.save(&shared.paths) {
                return Response::Error {
                    message: format!("applied for now, but could not save it: {e}"),
                };
            }
        }
        Err(e) => {
            return Response::Error {
                message: format!("applied for now, but could not read the config to save it: {e}"),
            };
        }
    }
    Response::Ok
}

/// Lists the modifier swaps: live sessions first, then saved-but-not-connected
/// hosts.
fn list_modifier_swaps(shared: &Arc<Shared>) -> Vec<ModifierInfo> {
    let state = shared.lock();
    let mut swaps = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for link in state.links.values() {
        seen.insert(link.host.clone());
        swaps.push(ModifierInfo {
            host: link.host.clone(),
            swap: link.modifier_swap.name().to_string(),
            connected: true,
        });
    }
    for (host, swap) in &state.modifier_swaps {
        if seen.contains(host) {
            continue;
        }
        swaps.push(ModifierInfo {
            host: host.clone(),
            swap: swap.name().to_string(),
            connected: false,
        });
    }
    swaps.sort_by(|a, b| a.host.cmp(&b.host));
    swaps
}

/// Parses an edge name. Accepts the four edges and the up/down synonyms.
fn parse_edge(name: &str) -> Option<Edge> {
    match name.trim().to_ascii_lowercase().as_str() {
        "left" => Some(Edge::Left),
        "right" => Some(Edge::Right),
        "top" | "up" => Some(Edge::Top),
        "bottom" | "down" => Some(Edge::Bottom),
        _ => None,
    }
}

/// The lowercase name of an edge, for display and the wire.
fn edge_name(edge: Edge) -> &'static str {
    match edge {
        Edge::Left => "left",
        Edge::Right => "right",
        Edge::Top => "top",
        Edge::Bottom => "bottom",
    }
}

/// Places a peer host past `edge`: records it for next time, persists it, and
/// applies it to any live session with that host right now.
fn set_layout(shared: &Arc<Shared>, host: &str, edge: &str) -> Response {
    let Some(edge) = parse_edge(edge) else {
        return Response::Error {
            message: format!("unknown edge '{edge}' — use left, right, top, or bottom"),
        };
    };

    {
        let mut state = shared.lock();
        state.placements.insert(host.to_string(), edge);
        // Apply to a live link with this host immediately, so the change is
        // visible without reconnecting.
        let live: Vec<MachineId> = state
            .links
            .iter()
            .filter(|(_, link)| link.host == host)
            .map(|(machine, _)| *machine)
            .collect();
        for machine in live {
            if let Some(link) = state.links.get_mut(&machine) {
                link.edge = edge;
            }
        }
        if !state.links.is_empty() {
            rebuild_layout(&mut state, shared);
            shared.sync_suppression(&state);
        }
    }
    notify_change(shared);

    // Persist to the config file (merging into whatever else is there).
    match Config::load(&shared.paths) {
        Ok(mut config) => {
            config.placements.insert(host.to_string(), edge);
            if let Err(e) = config.save(&shared.paths) {
                return Response::Error {
                    message: format!("placed for now, but could not save it: {e}"),
                };
            }
        }
        Err(e) => {
            return Response::Error {
                message: format!("placed for now, but could not read the config to save it: {e}"),
            };
        }
    }
    Response::Ok
}

/// Turns opt-in clipboard sharing on or off at runtime. Flips the manager's
/// guard so both directions honor it at once, wakes or parks the polling task,
/// and persists the choice to the config so it survives a restart.
fn set_clipboard(shared: &Arc<Shared>, enabled: bool) -> Response {
    shared.clipboard.set_enabled(enabled);
    // The task may have ended already (shutdown); the guard above is what
    // actually gates sharing, so a failed wake is harmless.
    let _ = shared.clipboard_on.send(enabled);
    notify_change(shared);

    match Config::load(&shared.paths) {
        Ok(mut config) => {
            config.clipboard_sharing_enabled = enabled;
            if let Err(e) = config.save(&shared.paths) {
                return Response::Error {
                    message: format!(
                        "clipboard sharing changed for now, but could not save it: {e}"
                    ),
                };
            }
        }
        Err(e) => {
            return Response::Error {
                message: format!(
                    "clipboard sharing changed for now, but could not read the config to save it: {e}"
                ),
            };
        }
    }
    Response::Ok
}

/// Lists the current placements: live sessions first, then saved-but-not-
/// connected hosts.
fn list_layout(shared: &Arc<Shared>) -> Vec<LayoutInfo> {
    let state = shared.lock();
    let mut placements = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for link in state.links.values() {
        seen.insert(link.host.clone());
        placements.push(LayoutInfo {
            host: link.host.clone(),
            edge: edge_name(link.edge).to_string(),
            connected: true,
        });
    }
    for (host, edge) in &state.placements {
        if seen.contains(host) {
            continue;
        }
        placements.push(LayoutInfo {
            host: host.clone(),
            edge: edge_name(*edge).to_string(),
            connected: false,
        });
    }
    placements.sort_by(|a, b| a.host.cmp(&b.host));
    placements
}

fn decide_pending(shared: &Arc<Shared>, selector: &str, approve: bool) -> Response {
    let mut state = shared.lock();
    let index = state
        .pending
        .iter()
        .position(|p| p.host == selector || p.fingerprint.to_string().starts_with(selector));
    match index {
        Some(index) => {
            let pending = state.pending.remove(index);
            let _ = pending.decision.send(approve);
            notify_change(shared);
            Response::Ok
        }
        None => Response::Error {
            message: format!("no pending request matches {selector}"),
        },
    }
}

fn status(shared: &Arc<Shared>) -> StatusInfo {
    let state = shared.lock();
    let active = state.sessions.active_target();
    StatusInfo {
        fingerprint: shared.local_fingerprint.to_string(),
        port: shared.port,
        capturing: shared.capturing.load(std::sync::atomic::Ordering::Relaxed),
        clipboard_sharing: shared.clipboard.is_enabled(),
        sessions: state
            .links
            .values()
            .map(|link| SessionInfo {
                host: link.host.clone(),
                fingerprint: link.fingerprint.to_string(),
                role: match link.role {
                    Role::Controller => "controller".into(),
                    Role::Target => "target".into(),
                },
                active: active == ActiveTarget::Remote(machine_id_of(link.fingerprint)),
            })
            .collect(),
        pending: state
            .pending
            .iter()
            .map(|p| PendingInfo {
                host: p.host.clone(),
                fingerprint: p.fingerprint.to_string(),
            })
            .collect(),
    }
}

fn list_peers(shared: &Arc<Shared>) -> Vec<PeerInfo> {
    let state = shared.lock();
    let connected: Vec<String> = state
        .links
        .values()
        .map(|l| l.fingerprint.to_string())
        .collect();
    shared
        .trust
        .peers()
        .into_iter()
        .map(|record| PeerInfo {
            connected: connected.contains(&record.fingerprint),
            host: record.host,
            fingerprint: record.fingerprint,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use omni_protocol::{Action, MouseButton};

    fn pointer(x: i32, y: i32) -> PeerCommand {
        PeerCommand::Input(InputEvent::Pointer { x, y })
    }

    fn click() -> PeerCommand {
        PeerCommand::Input(InputEvent::Button {
            button: MouseButton::Left,
            action: Action::Press,
        })
    }

    #[test]
    fn a_run_of_positions_collapses_to_the_latest() {
        let out = coalesce_motion(vec![pointer(1, 1), pointer(2, 2), pointer(3, 3)]);
        assert_eq!(out, vec![pointer(3, 3)]);
    }

    #[test]
    fn clicks_are_kept_and_break_the_run_so_they_land_in_place() {
        // pointer→pointer→click→pointer→pointer must keep the click at the
        // position it was made, then collapse the positions on each side.
        let out = coalesce_motion(vec![
            pointer(1, 1),
            pointer(2, 2),
            click(),
            pointer(3, 3),
            pointer(4, 4),
        ]);
        assert_eq!(out, vec![pointer(2, 2), click(), pointer(4, 4)]);
    }

    #[test]
    fn non_motion_commands_pass_through_untouched() {
        let out = coalesce_motion(vec![click(), click()]);
        assert_eq!(out, vec![click(), click()]);
    }

    #[test]
    fn an_empty_batch_stays_empty() {
        assert_eq!(coalesce_motion(vec![]), vec![]);
    }

    /// Records what was sent, taking its time over the first payload so that a
    /// later one would overtake it if sending were not sequential.
    fn recording_sender(
        recorded: Arc<std::sync::Mutex<Vec<String>>>,
    ) -> impl Fn(ClipboardData) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
    {
        move |data| {
            let recorded = recorded.clone();
            Box::pin(async move {
                let ClipboardData::Text(text) = data else {
                    return true;
                };
                if text == "first" {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                recorded.lock().unwrap().push(text);
                true
            })
        }
    }

    #[tokio::test]
    async fn clipboard_payloads_are_sent_one_after_another_in_order() {
        let (tx, rx) = mpsc::unbounded_channel();
        for text in ["first", "second", "third"] {
            tx.send(ClipboardData::Text(text.to_string())).unwrap();
        }
        drop(tx);

        let recorded = Arc::new(std::sync::Mutex::new(Vec::new()));
        run_clipboard_sender(rx, recording_sender(recorded.clone())).await;

        // A big copy takes a while to go out. The one made after it must still
        // arrive after it, or the peer ends up with an older clipboard.
        assert_eq!(*recorded.lock().unwrap(), vec!["first", "second", "third"]);
    }

    #[tokio::test]
    async fn sending_stops_once_the_connection_is_gone() {
        let (tx, rx) = mpsc::unbounded_channel();
        for text in ["first", "second"] {
            tx.send(ClipboardData::Text(text.to_string())).unwrap();
        }
        drop(tx);

        let recorded = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = recorded.clone();
        run_clipboard_sender(rx, move |data| {
            let seen = seen.clone();
            Box::pin(async move {
                if let ClipboardData::Text(text) = data {
                    seen.lock().unwrap().push(text);
                }
                false // the connection failed
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        })
        .await;

        // Nothing is attempted after a failure: the session is being torn down.
        assert_eq!(*recorded.lock().unwrap(), vec!["first"]);
    }
}
