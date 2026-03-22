use log::{error, info, warn};
use prost::Message;
use std::io;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};

use crate::network::TrackpadMessage;

pub struct OmnipresentServer {
    socket: UdpSocket,
    tx: mpsc::Sender<TrackpadMessage>,
    token: u32,
    last_sequence: u32,
    is_first_packet: bool,
}

impl OmnipresentServer {
    pub async fn bind(tx: mpsc::Sender<TrackpadMessage>, port: u16) -> io::Result<Self> {
        let address = format!("0.0.0.0:{}", port);

        let socket = match UdpSocket::bind(&address).await {
            Ok(s) => {
                info!("Server bound to port {}", port);
                s
            }
            Err(e) => {
                error!(
                    "Could not bind to port {}. It may already be in use. Error: {}",
                    port, e
                );
                return Err(e);
            }
        };

        Ok(Self {
            socket,
            tx,
            token: 0,
            last_sequence: 0,
            is_first_packet: true,
        })
    }

    pub fn set_token(&mut self, token: u32) {
        self.token = token;
    }

    pub async fn run(&mut self, mut shutdown_rx: watch::Receiver<bool>) -> io::Result<()> {
        let mut buf = [0u8; 1024];

        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    if changed.is_ok() && *shutdown_rx.borrow() {
                        info!("Core UDP server stopped");
                        return Ok(());
                    }
                }
                recv_result = self.socket.recv_from(&mut buf) => {
                    match recv_result {
                        Ok((len, peer)) => {
                            match TrackpadMessage::decode(&buf[..len]) {
                                Ok(msg) => {
                                    if msg.auth_token != self.token {
                                        warn!("Invalid token from {}. Rejecting packet.", peer.ip());
                                        let _ = self.socket.send_to(b"AUTH_FAIL", peer).await;
                                        continue;
                                    }

                                    if self.is_first_packet {
                                        info!("Client authenticated from {}", peer);
                                        let _ = self.socket.send_to(b"AUTH_OK", peer).await;
                                        self.is_first_packet = false;
                                    }

                                    let current_seq = msg.sequence_number;

                                    if !self.is_first_packet {
                                        let diff = current_seq.wrapping_sub(self.last_sequence);
                                        let is_old_packet = diff > (u32::MAX / 2);
                                        let is_duplicate = current_seq == self.last_sequence;

                                        if is_old_packet || is_duplicate {
                                            continue;
                                        }
                                    } else {
                                        self.is_first_packet = false;
                                    }

                                    self.last_sequence = current_seq;

                                    if let Err(e) = self.tx.try_send(msg) {
                                        match e {
                                            mpsc::error::TrySendError::Full(_) => {
                                                warn!("Input buffer is full. Dropping packet.");
                                            }
                                            mpsc::error::TrySendError::Closed(_) => break,
                                        }
                                    }
                                }
                                Err(e) => error!("Protobuf decode error: {}", e),
                            }
                        }
                        Err(e) => error!("UDP receive error: {}", e),
                    }
                }
            }
        }

        Ok(())
    }
}
