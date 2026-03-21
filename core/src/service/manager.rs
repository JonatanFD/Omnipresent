use crate::handler::controller::InputController;
use crate::mouse::factory::MouseStrategyFactory;
use crate::network::server::OmnipresentServer;
use crate::network::{ActionType, TrackpadMessage};
use crate::security::auth::AuthInfo;
use crate::service::models::{CoreConnectionInfo, CoreServiceConfig};
use local_ip_address::local_ip;
use log::error;
use std::io;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

pub struct RunningCoreService {
    connection: CoreConnectionInfo,
    shutdown_tx: watch::Sender<bool>,
    server_task: JoinHandle<io::Result<()>>,
}

impl RunningCoreService {
    pub async fn start(config: CoreServiceConfig) -> io::Result<Self> {
        let (tx, mut rx) = mpsc::channel::<TrackpadMessage>(100);
        let mut server = OmnipresentServer::bind(tx, config.port).await?;

        let token = AuthInfo::get_or_create_token(config.reset_pin);
        server.set_token(token);

        let ip = local_ip().ok().map(|value| value.to_string());
        let qr_payload = ip
            .as_ref()
            .map(|value| format!("omnipresent://{}:{}/?token={}", value, config.port, token));

        let connection = CoreConnectionInfo {
            ip,
            port: config.port,
            token,
            qr_payload,
        };

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let discovery_shutdown_rx = shutdown_rx.clone();

        tokio::spawn(async move {
            if let Err(err) =
                OmnipresentServer::start_discovery_service(config.port, token, discovery_shutdown_rx)
                    .await
            {
                error!("Discovery service failed: {}", err);
            }
        });

        std::thread::spawn(move || {
            let strategy = MouseStrategyFactory::create();
            let mut controller = InputController::new(strategy);

            while let Some(msg) = rx.blocking_recv() {
                let action = msg.action();
                let phase = msg.phase();

                if (msg.delta_x != 0.0 || msg.delta_y != 0.0) && action == ActionType::NoAction {
                    controller.move_mouse(msg.delta_x, msg.delta_y);
                }

                if action != ActionType::NoAction {
                    controller.execute_action(action, phase, msg.delta_x, msg.delta_y);
                }
            }
        });

        let server_task = tokio::spawn(async move { server.run(shutdown_rx).await });

        Ok(Self {
            connection,
            shutdown_tx,
            server_task,
        })
    }

    pub fn connection_info(&self) -> &CoreConnectionInfo {
        &self.connection
    }

    pub fn is_running(&self) -> bool {
        !self.server_task.is_finished()
    }

    pub async fn wait(self) -> io::Result<()> {
        self.server_task
            .await
            .map_err(|err| io::Error::other(err.to_string()))?
    }

    pub async fn stop(self) -> io::Result<()> {
        let _ = self.shutdown_tx.send(true);
        self.server_task
            .await
            .map_err(|err| io::Error::other(err.to_string()))?
    }
}
