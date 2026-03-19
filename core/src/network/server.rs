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
    // 1. Modificamos bind para recibir el puerto deseado
    pub async fn bind(tx: mpsc::Sender<TrackpadMessage>, port: u16) -> io::Result<Self> {
        let address = format!("0.0.0.0:{}", port);

        // 2. Intentamos usar ESE puerto estrictamente
        let socket = match UdpSocket::bind(&address).await {
            Ok(s) => {
                info!("Servidor enlazado exitosamente al puerto fijo: {}", port);
                s
            }
            Err(e) => {
                error!(
                    "FATAL: No se pudo usar el puerto {}. ¿Otra app lo está usando? Error: {}",
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

    // (Opcional) Puedes eliminar get_assigned_port() ya que tú defines el puerto ahora.

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
                            // Verificación de Seguridad con el token fijo
                            if msg.auth_token != self.token {
                                warn!("Token inválido desde {}. Bloqueando.", peer.ip());
                                continue;
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
                                        warn!("Buffer lleno, descartando paquete")
                                    }
                                    mpsc::error::TrySendError::Closed(_) => break,
                                }
                            }
                        }
                        Err(e) => error!("Error Protobuf: {}", e),
                    }
                }
                Err(e) => error!("Error UDP: {}", e),
            }
        }
        Ok(())
    }
}
