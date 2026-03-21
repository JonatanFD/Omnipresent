use crate::services::core_service::CoreServiceHandle;
use tokio::sync::Mutex;

pub struct AppState {
    pub core_service: Mutex<CoreServiceHandle>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            core_service: Mutex::new(CoreServiceHandle::new()),
        }
    }
}
