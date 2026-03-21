use omnipresent_core::service::manager::RunningCoreService;
use omnipresent_core::service::models::{CoreConnectionInfo, CoreServiceConfig};
use serde::Serialize;
use std::io;

#[derive(Clone, Debug, Serialize)]
pub struct CoreServiceStatus {
    pub running: bool,
    pub connection: Option<CoreConnectionInfo>,
}

pub struct CoreServiceHandle {
    running: Option<RunningCoreService>,
}

impl CoreServiceHandle {
    pub fn new() -> Self {
        Self { running: None }
    }

    pub async fn start(&mut self, port: u16, reset_pin: bool) -> io::Result<CoreServiceStatus> {
        if let Some(service) = self.running.take() {
            let current_port = service.connection_info().port;
            if !reset_pin && current_port == port && service.is_running() {
                self.running = Some(service);
                return Ok(self.status());
            }

            service.stop().await?;
        }

        let service = RunningCoreService::start(CoreServiceConfig { port, reset_pin }).await?;
        let connection = service.connection_info().clone();
        self.running = Some(service);

        Ok(CoreServiceStatus {
            running: true,
            connection: Some(connection),
        })
    }

    pub async fn stop(&mut self) -> io::Result<CoreServiceStatus> {
        if let Some(service) = self.running.take() {
            service.stop().await?;
        }

        Ok(self.status())
    }

    pub fn status(&self) -> CoreServiceStatus {
        match &self.running {
            Some(service) if service.is_running() => CoreServiceStatus {
                running: true,
                connection: Some(service.connection_info().clone()),
            },
            _ => CoreServiceStatus {
                running: false,
                connection: None,
            },
        }
    }
}
