mod handler;
mod mouse;
mod network;
mod security;

use crate::handler::controller::InputController;
use crate::mouse::factory::MouseStrategyFactory;
use crate::network::server::OmnipresentServer;
use crate::network::{ActionType, TrackpadMessage};
use crate::security::auth::AuthInfo;
use env_logger::Env;
use log::info;
use std::io;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("Starting Omnipresent Server...");
    // Create a channel for the server to send messages to the handler thread
    let (tx, mut rx) = mpsc::channel::<TrackpadMessage>(100);

    // Bind the server to the channel
    let mut server = OmnipresentServer::bind(tx).await?;

    // Get the assigned port from the server
    let assigned_port = server.get_assigned_port()?;

    // Generate authentication info for the assigned port
    let auth_info = AuthInfo::generate(assigned_port);

    server.set_token(auth_info.token);

    std::thread::spawn(move || {
        // 1. Use the MouseStrategyFactory to get the correct strategy for the OS
        let strategy = MouseStrategyFactory::create();

        // 2. Inject the strategy into the controller
        let mut controller = InputController::new(strategy);

        info!("Omnipresent Server Started");

        while let Some(msg) = rx.blocking_recv() {
            let action = msg.action();
            let phase = msg.phase();

            info!(
                "Received event - dx: {:.2}, dy: {:.2}, action: {:?}, phase: {:?}",
                msg.delta_x,
                msg.delta_y,
                msg.action(),
                msg.phase()
            );

            // Separamos la ejecución: Movimiento puro vs Acciones (Clics, Scrolls, Swipes)
            if (msg.delta_x != 0.0 || msg.delta_y != 0.0) && action == ActionType::NoAction {
                controller.move_mouse(msg.delta_x, msg.delta_y);
            }

            if action != ActionType::NoAction {
                controller.execute_action(action, phase, msg.delta_x, msg.delta_y);
            }
        }
    });

    server.run().await
}
