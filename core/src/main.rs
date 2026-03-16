mod handler;
mod mouse;
mod network;

use crate::handler::controller::InputController;
use crate::mouse::factory::MouseStrategyFactory;
use crate::network::TrackpadMessage;
use network::server::OmnipresentServer;
use std::io;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let (tx, mut rx) = mpsc::channel::<TrackpadMessage>(100);

    std::thread::spawn(move || {
        // 1. Use the MouseStrategyFactory to get the correct strategy for the OS
        let strategy = MouseStrategyFactory::create();

        // 2. Inject the strategy into the controller
        let mut controller = InputController::new(strategy);

        let mut last_timestamp = 0;

        while let Some(msg) = rx.blocking_recv() {
            if msg.timestamp < last_timestamp {
                continue;
            }
            last_timestamp = msg.timestamp;

            if msg.delta_x != 0.0 || msg.delta_y != 0.0 {
                controller.move_mouse(msg.delta_x, msg.delta_y);
            }

            if msg.action != 0 {
                controller.execute_action(msg.action, msg.phase);
            }
        }
    });

    let server = OmnipresentServer::new(8080, tx);
    server.run().await
}
