//! The desktop client.
//!
//! The daemon runs inside this process, so installing the app is the whole
//! installation — there is no second thing to set up, and the `omni` command
//! ships with it (see [`cli`]). The window is still only a client of the
//! daemon's IPC surface (see [`ipc`]); embedding the daemon changes where it
//! runs, not who owns the state.

mod cli;
mod daemon;
mod diagnostics;
mod ipc;
mod notify;
mod tray;

use tauri::{Manager, WindowEvent};

use daemon::Supervisor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must run before anything queries the screen or installs input hooks, so the
    // window and the daemon agree on one coordinate space on a high-DPI display.
    omni_runtime::prepare_process();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Supervisor::default())
        .setup(|app| {
            let handle = app.handle();

            tray::create(handle)?;
            ipc::spawn_subscription(handle.clone());

            // A failure here is not fatal, and usually is not even a failure: a
            // daemon started by `omni start`, or by another copy of this app,
            // owns the socket and ours stands down. The window then talks to the
            // one that is there, which is what we want. Either way the outcome
            // arrives as an event rather than being swallowed.
            let _ = app.state::<Supervisor>().start(handle.clone());

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
            daemon::daemon_start,
            daemon::daemon_embedded,
            diagnostics::daemon_doctor,
            cli::cli_status,
            cli::cli_install,
            cli::app_version,
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
