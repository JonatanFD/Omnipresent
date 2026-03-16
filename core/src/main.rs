mod network;

use network::server::OmnipresentServer;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let server = OmnipresentServer::new(8080);

    server.run().await
}
