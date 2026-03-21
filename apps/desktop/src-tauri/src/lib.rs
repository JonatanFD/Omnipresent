mod commands;
mod services;
mod state;

use commands::core_commands::{
    get_core_status, get_current_wifi_info, start_core_service, stop_core_service,
};
use state::app_state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            start_core_service,
            stop_core_service,
            get_core_status,
            get_current_wifi_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
