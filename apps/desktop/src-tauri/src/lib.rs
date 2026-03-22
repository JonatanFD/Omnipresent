mod commands;
mod services;
mod state;
mod tray;

use commands::core_commands::{
    get_core_status, get_current_wifi_info, start_core_service, stop_core_service,
};
use state::app_state::AppState;
use tray::system_tray::create_system_tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .setup(|app| {
            create_system_tray(&app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            start_core_service,
            stop_core_service,
            get_core_status,
            get_current_wifi_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
