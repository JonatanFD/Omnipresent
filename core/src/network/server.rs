use log::{error, info, warn};
use prost::Message;
use std::io;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::network::TrackpadMessage;

pub struct OmnipresentServer {
    socket: UdpSocket,
    tx: mpsc::Sender<TrackpadMessage>,
    token: u32,
}

impl OmnipresentServer {
    pub async fn bind(tx: mpsc::Sender<TrackpadMessage>) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        Ok(Self {
            socket,
            tx,
            token: 0,
        })
    }

    pub fn get_assigned_port(&self) -> io::Result<u16> {
        let addr = self.socket.local_addr()?;
        Ok(addr.port())
    }

    pub fn set_token(&mut self, token: u32) {
        self.token = token;
    }

    pub async fn run(&self) -> io::Result<()> {
        let mut buf = [0u8; 1024];

        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((len, peer)) => match TrackpadMessage::decode(&buf[..len]) {
                    Ok(msg) => {
                        if msg.auth_token != self.token {
                            warn!(
                                "Blocking unauthorized packet from {}. Invalid token.",
                                peer.ip()
                            );
                            continue;
                        }

                        info!(
                            "Received event - dx: {:.2}, dy: {:.2}, action: {:?}, phase: {:?}",
                            msg.delta_x,
                            msg.delta_y,
                            msg.action(),
                            msg.phase()
                        );

                        if let Err(e) = self.tx.send(msg).await {
                            error!("Channel receiver closed: {}", e);
                            break;
                        }
                    }
                    Err(e) => error!("Error decoding Protobuf: {}", e),
                },
                Err(e) => error!("Error receiving from UDP: {}", e),
            }
        }
        Ok(())
    }
}
