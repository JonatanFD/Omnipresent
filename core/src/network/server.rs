use log::{error, info};
use prost::Message;
use std::io;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::network::TrackpadMessage;

pub struct OmnipresentServer {
    port: u16,
    tx: mpsc::Sender<TrackpadMessage>,
}

impl OmnipresentServer {
    pub fn new(port: u16, tx: mpsc::Sender<TrackpadMessage>) -> Self {
        Self { port, tx }
    }

    pub async fn run(&self) -> io::Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let socket = UdpSocket::bind(&addr).await?;
        info!("UDP server listening on {}", addr);

        let mut buf = [0u8; 1024];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, _peer)) => match TrackpadMessage::decode(&buf[..len]) {
                    Ok(msg) => {
                        if let Err(e) = self.tx.send(msg).await {
                            error!("Channel receiver closed: {}", e);
                            break;
                        }
                    }
                    Err(e) => error!("Error decoding Protobuf message: {}", e),
                },
                Err(e) => error!("Error receiving UDP packet: {}", e),
            }
        }
        Ok(())
    }
}
