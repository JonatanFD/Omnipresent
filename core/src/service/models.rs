use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub struct CoreServiceConfig {
    pub port: u16,
    pub reset_pin: bool,
}

impl Default for CoreServiceConfig {
    fn default() -> Self {
        Self {
            port: 9090,
            reset_pin: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CoreConnectionInfo {
    pub ip: Option<String>,
    pub port: u16,
    pub token: u32,
    pub qr_payload: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum ServiceStatus {
    Stopped,
    Running,
}
