//! The system-tray entry.
//!
//! Sharing a keyboard and mouse is a background job, so closing the window must
//! not stop it. The window hides instead of quitting (see `lib.rs`) and this is
//! how the user gets it back — or quits for real when they mean to.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

/// Builds the tray icon and its menu. Called once, during setup.
pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Omnipresent", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Omnipresent", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &separator, &quit])?;

    let mut builder = TrayIconBuilder::with_id("omni")
        .menu(&menu)
        .tooltip("Omnipresent")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            // The daemon lives in this process, so quitting stops input sharing
            // too. That is what the user is asking for by choosing Quit.
            "quit" => app.exit(0),
            _ => {}
        })
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

/// Brings the main window back and focuses it.
fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
