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
    is_first_packet: bool, // Para permitir el primer paquete sin importar su número
}

impl OmnipresentServer {
    pub async fn bind(tx: mpsc::Sender<TrackpadMessage>) -> io::Result<Self> {
        // Enlaza a cualquier puerto disponible (puerto 0)
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        Ok(Self {
            socket,
            tx,
            token: 0,
            last_sequence: 0,
            is_first_packet: true,
        })
    }

    pub fn get_assigned_port(&self) -> io::Result<u16> {
        let addr = self.socket.local_addr()?;
        Ok(addr.port())
    }

    pub fn set_token(&mut self, token: u32) {
        self.token = token;
    }

    /// Ejecuta el servidor UDP.
    /// Este bucle es infinito y procesa los paquetes de entrada.
    pub async fn run(&mut self) -> io::Result<()> {
        let mut buf = [0u8; 1024];

        info!(
            "Servidor UDP escuchando en el puerto {}",
            self.get_assigned_port()?
        );

        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((len, peer)) => {
                    match TrackpadMessage::decode(&buf[..len]) {
                        Ok(msg) => {
                            // 1. Verificación de Seguridad
                            if msg.auth_token != self.token {
                                warn!("Token inválido desde {}. Bloqueando paquete.", peer.ip());
                                continue;
                            }

                            // 2. Filtro Anti-Jitter (Secuencia UDP)
                            let current_seq = msg.sequence_number;

                            if !self.is_first_packet {
                                let diff = current_seq.wrapping_sub(self.last_sequence);

                                // Si la diferencia es mayor a la mitad del rango de u32,
                                // significa que el paquete es antiguo (out-of-order).
                                let is_old_packet = diff > (u32::MAX / 2);
                                let is_duplicate = current_seq == self.last_sequence;

                                if is_old_packet || is_duplicate {
                                    // Ignoramos paquetes viejos o duplicados para evitar saltos del cursor
                                    continue;
                                }
                            } else {
                                self.is_first_packet = false;
                            }

                            self.last_sequence = current_seq;

                            // 3. Despacho al controlador de Mouse
                            // Usamos try_send para que la red nunca se bloquee si el controlador de mouse tarda.
                            // Es preferible perder un paquete de movimiento que acumular lag.
                            if let Err(e) = self.tx.try_send(msg) {
                                match e {
                                    mpsc::error::TrySendError::Full(_) => {
                                        warn!(
                                            "Buffer de entrada lleno: descartando paquete para mantener baja latencia"
                                        );
                                    }
                                    mpsc::error::TrySendError::Closed(_) => {
                                        error!(
                                            "El canal de procesamiento se ha cerrado. Deteniendo servidor."
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => error!("Error al decodificar Protobuf: {}", e),
                    }
                }
                Err(e) => {
                    error!("Error en la recepción UDP: {}", e);
                    // Opcional: break si el error es fatal
                }
            }
        }

        // El Ok(()) es alcanzable solo si el canal se cierra y salimos del loop con 'break'
        Ok(())
    }
}
