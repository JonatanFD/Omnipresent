use log::{debug, error, info, warn};
use prost::Message;
use std::io;
use tokio::net::UdpSocket;

pub mod network {
    include!(concat!(env!("OUT_DIR"), "/network_protocol.rs"));
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let addr = "127.0.0.1:8080";

    // Starting UDP server
    let socket = match UdpSocket::bind(addr).await {
        Ok(s) => {
            info!("UDP Server Started Successfully on {}", addr);
            s
        }
        Err(e) => {
            error!("Could not bind socket to {}: {}", addr, e);
            return Err(e);
        }
    };

    let mut buf = [0u8; 1024];

    info!("Waiting for packages (ctrl + c to exit)");

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, peer)) => {
                let data = &buf[..len];
                debug!("Package received from {} with {} bytes", peer, len);

                let msg = String::from_utf8_lossy(data);

                info!("[{}] Message: {}", peer, msg);

                let response = format!("Confirmed: {}", msg);
                if let Err(e) = socket.send_to(response.as_bytes(), peer).await {
                    warn!("Could not respond to {}: {}", peer, e);
                }
            }
            Err(e) => {
                error!("Error getting data: {}", e);
            }
        }
    }
}
