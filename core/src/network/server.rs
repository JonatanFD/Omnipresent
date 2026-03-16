use log::{debug, error, info};
use std::io;
use std::sync::Arc;
use tokio::net::UdpSocket;

pub struct OmnipresentServer {
    port: u16,
}

impl OmnipresentServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn run(&self) -> io::Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let socket = UdpSocket::bind(&addr).await?;

        info!("🚀 Omnipresent Server listening on {}", addr);

        let socket = Arc::new(socket);
        let mut buf = [0u8; 1024];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, peer)) => {
                    let data = buf[..len].to_vec();
                    debug!("Received {} bytes from {}", len, peer);

                    tokio::spawn(async move {
                        Self::handle_packet(data, peer).await;
                    });
                }
                Err(e) => {
                    error!("Error receiving packet: {}", e);
                }
            }
        }
    }

    async fn handle_packet(data: Vec<u8>, peer: std::net::SocketAddr) {
        // En el futuro, aquí llamarás a tu Decoder de Protobuf
        info!("Processing packet from {}: {} bytes", peer, data.len());
    }
}
