//! The desktop client.
//!
//! The daemon runs inside this process, so installing the app is the whole
//! installation — there is no second thing to set up. The window is still only a
//! client of the daemon's IPC surface (see [`ipc`]); embedding the daemon changes
//! where it runs, not who owns the state.

mod ipc;
mod tray;

use tauri::WindowEvent;

/// Starts the daemon on a background thread.
///
/// A failure here is not fatal. The usual cause is that a daemon is already
/// running — started by `omni start` or by another copy of this app — and it owns
/// the socket. In that case the window simply talks to the one that is already
/// there, which is the behaviour we want anyway.
fn start_daemon() {
    std::thread::spawn(|| {
        if let Err(error) = omni_runtime::run() {
            // The daemon writes its own log; this only notes why the embedded one
            // stood down. It never carries key material.
            eprintln!("omni: the embedded daemon did not start ({error:?})");
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must run before anything queries the screen or installs input hooks, so the
    // window and the daemon agree on one coordinate space on a high-DPI display.
    omni_runtime::prepare_process();

    start_daemon();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            tray::create(app.handle())?;
            ipc::spawn_subscription(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window hides it instead of quitting: input sharing has
            // to survive it. Quit is on the tray menu, where the user can mean it.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            ipc::daemon_status,
            ipc::daemon_hello,
            ipc::daemon_stop,
            ipc::peer_connect,
            ipc::peer_disconnect,
            ipc::peer_accept,
            ipc::peer_reject,
            ipc::peer_list,
            ipc::peer_remove,
            ipc::layout_list,
            ipc::layout_set,
            ipc::modifiers_list,
            ipc::modifiers_set,
            ipc::clipboard_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
