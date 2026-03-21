use env_logger::Env;
use log::{error, info};
use omnipresent_core::service::manager::RunningCoreService;
use omnipresent_core::service::models::CoreServiceConfig;
use std::env;
use std::io;

const SERVER_PORT: u16 = 9090;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    info!("Starting Omnipresent server");

    let args: Vec<String> = env::args().collect();
    let force_reset = args.iter().any(|arg| arg == "--reset-pin");

    let service = RunningCoreService::start(CoreServiceConfig {
        port: SERVER_PORT,
        reset_pin: force_reset,
    })
    .await?;

    let connection = service.connection_info();
    match &connection.ip {
        Some(ip) => info!(
            "Server is ready. Listening on {}:{} with PIN {}",
            ip, connection.port, connection.token
        ),
        None => {
            error!("Could not resolve local IP");
            info!(
                "Server is ready. Listening on port {} with PIN {}",
                connection.port, connection.token
            );
        }
    }

    service.wait().await
}
