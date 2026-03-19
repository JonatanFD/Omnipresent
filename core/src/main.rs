mod handler;
mod mouse;
mod network;
mod security;

use crate::handler::controller::InputController;
use crate::mouse::factory::MouseStrategyFactory;
use crate::network::server::OmnipresentServer;
use crate::network::{ActionType, TrackpadMessage};
use crate::security::auth::AuthInfo; // Importamos el módulo actualizado
use env_logger::Env;
use log::info;
use std::env;
use std::io;
use tokio::sync::mpsc;

// Puerto fijo garantizado
const SERVER_PORT: u16 = 9090;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("Iniciando Omnipresent Server...");

    // Verificamos si el usuario pasó el argumento "--reset-pin" al iniciar la app
    let args: Vec<String> = env::args().collect();
    let force_reset = args.contains(&String::from("--reset-pin"));

    if force_reset {
        info!("Se detectó la bandera --reset-pin. Forzando regeneración de credenciales...");
    }

    let (tx, mut rx) = mpsc::channel::<TrackpadMessage>(100);

    // 1. Iniciamos el servidor con el puerto fijo
    let mut server = OmnipresentServer::bind(tx, SERVER_PORT).await?;

    // 2. Cargamos el PIN guardado o creamos uno nuevo
    let secure_pin = AuthInfo::get_or_create_token(force_reset);
    server.set_token(secure_pin);

    info!(
        "Esperando conexiones en el puerto {} con el PIN: {}",
        SERVER_PORT, secure_pin
    );

    std::thread::spawn(move || {
        let strategy = MouseStrategyFactory::create();
        let mut controller = InputController::new(strategy);

        info!("Controlador de Input Iniciado");

        while let Some(msg) = rx.blocking_recv() {
            let action = msg.action();
            let phase = msg.phase();

            if (msg.delta_x != 0.0 || msg.delta_y != 0.0) && action == ActionType::NoAction {
                controller.move_mouse(msg.delta_x, msg.delta_y);
            }

            if action != ActionType::NoAction {
                controller.execute_action(action, phase, msg.delta_x, msg.delta_y);
            }
        }
    });

    server.run().await
}
