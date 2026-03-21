use crate::services::core_service::CoreServiceStatus;
use crate::services::wifi_service::{WifiInfo, get_current_wifi};
use crate::state::app_state::AppState;
use tauri::State;

#[tauri::command]
pub async fn start_core_service(
    state: State<'_, AppState>,
    port: Option<u16>,
    reset_pin: Option<bool>,
) -> Result<CoreServiceStatus, String> {
    let mut handle = state.core_service.lock().await;

    handle
        .start(port.unwrap_or(9090), reset_pin.unwrap_or(false))
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn stop_core_service(state: State<'_, AppState>) -> Result<CoreServiceStatus, String> {
    let mut handle = state.core_service.lock().await;

    handle.stop().await.map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_core_status(state: State<'_, AppState>) -> Result<CoreServiceStatus, String> {
    let handle = state.core_service.blocking_lock();
    Ok(handle.status())
}

#[tauri::command]
pub fn get_current_wifi_info() -> Result<WifiInfo, String> {
    Ok(get_current_wifi())
}
