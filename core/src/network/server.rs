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
    last_sequence: u32,
    is_first_packet: bool,
}

impl OmnipresentServer {
    pub async fn start_discovery_service(port: u16, token: u32) -> io::Result<()> {
        let discovery_port = port + 1;

        // On Windows, instead of binding to 0.0.0.0, try to get the actual Wi-Fi IP
        let bind_ip = match local_ip_address::local_ip() {
            Ok(ip) => ip.to_string(),
            // Fallback to 0.0.0.0 only if the local IP lookup fails
            Err(_) => "0.0.0.0".to_string(),
        };

        let address = format!("{}:{}", bind_ip, discovery_port);

        // Bind to the specific interface first, then fall back to the traditional 0.0.0.0 binding
        let socket = match UdpSocket::bind(&address).await {
            Ok(s) => s,
            Err(_) => UdpSocket::bind(format!("0.0.0.0:{}", discovery_port)).await?,
        };

        if let Err(e) = socket.set_broadcast(true) {
            warn!("Could not enable broadcast mode: {}", e);
        }

        info!(
            "Service discovery active on IP {} (Port {})",
            bind_ip, discovery_port
        );

        let mut buf = [0u8; 1024];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, peer)) => {
                    let message = String::from_utf8_lossy(&buf[..len]);

                    info!(
                        "[DISCOVERY] Discovery request received from {}: '{}'",
                        peer, message
                    );

                    if message == "OMNIPRESENT_DISCOVERY" {
                        let response = format!("OMNIPRESENT_HERE|{}|{}", port, token);
                        let _ = socket.send_to(response.as_bytes(), peer).await;
                        info!("[DISCOVERY] Discovery response sent to {}", peer);
                    }
                }
                Err(e) => {
                    error!("Discovery service error: {}", e);
                }
            }
        }
    }

    // Bind the UDP server to the specified port
    pub async fn bind(tx: mpsc::Sender<TrackpadMessage>, port: u16) -> io::Result<Self> {
        let address = format!("0.0.0.0:{}", port);

        // Try to bind strictly to the requested port
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

    pub async fn run(&mut self) -> io::Result<()> {
        let mut buf = [0u8; 1024];

        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((len, peer)) => {
                    match TrackpadMessage::decode(&buf[..len]) {
                        Ok(msg) => {
                            // Security check using the fixed token
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
        Ok(())
    }
}
