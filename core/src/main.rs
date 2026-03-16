mod handler;
mod network;

use crate::handler::controller::InputController;
use crate::network::TrackpadMessage;
use network::server::OmnipresentServer;
use std::io;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    // Create a channel to communicate the network (async) with the hardware layer (sync)
    let (tx, mut rx) = mpsc::channel::<TrackpadMessage>(100);

    // Dedicated thread to move the mouse without blocking the server
    std::thread::spawn(move || {
        let mut controller = InputController::new();
        let mut last_timestamp = 0;

        while let Some(msg) = rx.blocking_recv() {
            // Ignore out-of-order packets
            if msg.timestamp < last_timestamp {
                continue;
            }
            last_timestamp = msg.timestamp;

            // 1. Handle movement
            if msg.delta_x != 0.0 || msg.delta_y != 0.0 {
                controller.move_mouse(msg.delta_x, msg.delta_y);
            }

            // 2. Handle actions
            // Note: we cast to i32 because Prost enums are internally represented as i32
            if msg.action != 0 {
                controller.execute_action(msg.action, msg.phase);
            }
        }
    });

    // Start the UDP server
    let server = OmnipresentServer::new(8080, tx);
    server.run().await
}
