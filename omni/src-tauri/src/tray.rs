//! The system-tray entry.
//!
//! Sharing a keyboard and mouse is a background job, so closing the window must
//! not stop it. The window hides instead of quitting (see `lib.rs`) and this is
//! how the user gets it back — or quits for real when they mean to.
//!
//! It is also where an incoming request has to be answerable. A closed window is
//! the normal state for a tray app, and that is exactly when a peer asks for
//! control: the menu therefore carries the waiting requests, so nobody has to
//! reopen the window to let a machine in.

use omni_runtime::ipc::{Request, StatusInfo};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::daemon::Supervisor;
use crate::ipc::request;

/// The tray's id, so the icon can be found again to update its menu.
const TRAY_ID: &str = "omni";

/// Prefixes for the per-request menu entries. The fingerprint follows, because
/// it is what identifies a peer even before it has a name worth trusting.
const ACCEPT_PREFIX: &str = "accept:";
const REJECT_PREFIX: &str = "reject:";

/// Builds the tray icon and its menu. Called once, during setup.
pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu(app, &[])?)
        .tooltip("Omnipresent")
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            // Left click reopens the window, which is what both Windows and macOS
            // users expect from a tray/menu-bar item.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

/// One waiting request, as the menu needs it.
pub struct Waiting {
    pub host: String,
    pub fingerprint: String,
}

/// Rebuilds the menu and tooltip for the requests currently waiting.
///
/// Called on every pushed snapshot. Rebuilding unconditionally keeps this
/// honest with no state of its own: the menu is a function of the daemon's
/// snapshot, never a running edit of what was there before.
pub fn sync<R: Runtime>(app: &AppHandle<R>, status: &StatusInfo) {
    let waiting: Vec<Waiting> = status
        .pending
        .iter()
        .map(|request| Waiting {
            host: request.host.clone(),
            fingerprint: request.fingerprint.clone(),
        })
        .collect();

    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    if let Ok(menu) = menu(app, &waiting) {
        let _ = tray.set_menu(Some(menu));
    }
    let _ = tray.set_tooltip(Some(tooltip(waiting.len())));
}

/// What the tray says on hover. The count is here because it is the only part
/// visible without opening the menu.
fn tooltip(waiting: usize) -> String {
    match waiting {
        0 => "Omnipresent".into(),
        1 => "Omnipresent — 1 request waiting".into(),
        n => format!("Omnipresent — {n} requests waiting"),
    }
}

fn menu<R: Runtime>(app: &AppHandle<R>, waiting: &[Waiting]) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;

    for request in waiting {
        // The host names the machine, but the fingerprint is what gets pinned,
        // so the id carries the fingerprint and the label carries the name.
        let accept = MenuItem::with_id(
            app,
            format!("{ACCEPT_PREFIX}{}", request.fingerprint),
            format!("Accept {}", request.host),
            true,
            None::<&str>,
        )?;
        let reject = MenuItem::with_id(
            app,
            format!("{REJECT_PREFIX}{}", request.fingerprint),
            format!("Reject {}", request.host),
            true,
            None::<&str>,
        )?;
        menu.append(&accept)?;
        menu.append(&reject)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }

    menu.append(&MenuItem::with_id(
        app,
        "open",
        "Open Omnipresent",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        "Quit Omnipresent",
        true,
        None::<&str>,
    )?)?;

    Ok(menu)
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    let id = event.id.as_ref();

    if let Some(fingerprint) = id.strip_prefix(ACCEPT_PREFIX) {
        let _ = request(Request::Accept {
            selector: fingerprint.to_string(),
        });
        return;
    }
    if let Some(fingerprint) = id.strip_prefix(REJECT_PREFIX) {
        let _ = request(Request::Reject {
            selector: fingerprint.to_string(),
        });
        return;
    }

    match id {
        "open" => show_window(app),
        "quit" => quit(app),
        _ => {}
    }
}

/// Quits for real.
///
/// The daemon lives in this process, so quitting stops input sharing — that is
/// what the user is asking for by choosing Quit. It is stopped through its own
/// IPC first, so it closes its sessions and releases its socket rather than
/// being cut off mid-flight. A daemon that was already running when the app
/// started is left alone: this app never started it, so it is not its to stop.
fn quit<R: Runtime>(app: &AppHandle<R>) {
    if app.state::<Supervisor>().is_embedded() {
        let _ = request(Request::Stop);
    }
    app.exit(0);
}

/// Brings the main window back and focuses it.
fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tooltip_is_plain_when_nothing_is_waiting() {
        assert_eq!(tooltip(0), "Omnipresent");
    }

    #[test]
    fn one_waiting_request_reads_as_singular() {
        assert_eq!(tooltip(1), "Omnipresent — 1 request waiting");
    }

    #[test]
    fn several_waiting_requests_are_counted() {
        assert_eq!(tooltip(3), "Omnipresent — 3 requests waiting");
    }

    #[test]
    fn a_menu_id_round_trips_the_fingerprint_it_carries() {
        // The id is what the click handler acts on, so the fingerprint has to
        // come back out of it exactly — a truncated one would accept a peer the
        // user never saw.
        let fingerprint = "9f2c4a".repeat(10);
        let id = format!("{ACCEPT_PREFIX}{fingerprint}");

        assert_eq!(id.strip_prefix(ACCEPT_PREFIX), Some(fingerprint.as_str()));
        assert_eq!(id.strip_prefix(REJECT_PREFIX), None);
    }

    #[test]
    fn accept_and_reject_ids_cannot_be_confused() {
        let fingerprint = "abc123";

        let accept = format!("{ACCEPT_PREFIX}{fingerprint}");
        let reject = format!("{REJECT_PREFIX}{fingerprint}");

        assert!(accept.strip_prefix(REJECT_PREFIX).is_none());
        assert!(reject.strip_prefix(ACCEPT_PREFIX).is_none());
    }

    #[test]
    fn the_fixed_entries_are_not_mistaken_for_a_request() {
        // "open" and "quit" go through the same handler as the request ids.
        for id in ["open", "quit"] {
            assert!(id.strip_prefix(ACCEPT_PREFIX).is_none());
            assert!(id.strip_prefix(REJECT_PREFIX).is_none());
        }
    }
}
