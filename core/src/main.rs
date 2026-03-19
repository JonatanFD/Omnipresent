mod handler;
mod mouse;
mod network;
mod security;

use crate::handler::controller::InputController;
use crate::mouse::factory::MouseStrategyFactory;
use crate::network::server::OmnipresentServer;
use crate::network::{ActionType, TrackpadMessage};
use crate::security::auth::AuthInfo;
use env_logger::Env;
use log::{error, info};
use std::env;
use std::io;
use tokio::sync::mpsc;

// Librerías para el Popup del QR
use image::{Luma, Rgb};
use local_ip_address::local_ip;
use qrcode::QrCode;

// Puerto fijo garantizado
const SERVER_PORT: u16 = 9090;

#[tokio::main]
async fn main() -> io::Result<()> {
    // Inicializamos el logger
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("Iniciando Omnipresent Server...");

    // Verificamos si se pidió resetear el PIN desde la terminal
    let args: Vec<String> = env::args().collect();
    let force_reset = args.contains(&String::from("--reset-pin"));

    // Canal de comunicación entre el servidor UDP y el controlador del mouse
    let (tx, mut rx) = mpsc::channel::<TrackpadMessage>(100);

    // 1. Iniciamos el servidor con el puerto fijo
    let mut server = OmnipresentServer::bind(tx, SERVER_PORT).await?;

    // 2. Cargamos el PIN guardado (o creamos uno nuevo con SystemTime)
    let secure_pin = AuthInfo::get_or_create_token(force_reset);
    server.set_token(secure_pin);

    // 3. Obtener IP y Mostrar el QR como ventana emergente (Popup)
    match local_ip() {
        Ok(my_local_ip) => {
            info!("IP Local detectada: {}", my_local_ip);
            // Llamamos a la función que crea y abre la imagen PNG
            show_qr_popup(&my_local_ip.to_string(), SERVER_PORT, secure_pin);
        }
        Err(e) => {
            error!("No se pudo obtener la IP local para el QR: {}", e);
            info!(
                "Conéctate usando el puerto {} y PIN {}",
                SERVER_PORT, secure_pin
            );
        }
    }

    // 4. Arrancamos el hilo del controlador del mouse
    std::thread::spawn(move || {
        let strategy = MouseStrategyFactory::create();
        let mut controller = InputController::new(strategy);

        info!("Controlador de Input Iniciado. Escuchando movimientos...");

        while let Some(msg) = rx.blocking_recv() {
            let action = msg.action();
            let phase = msg.phase();

            // Movimiento puro
            if (msg.delta_x != 0.0 || msg.delta_y != 0.0) && action == ActionType::NoAction {
                controller.move_mouse(msg.delta_x, msg.delta_y);
            }

            // Acciones (Clics, Scrolls, Swipes)
            if action != ActionType::NoAction {
                controller.execute_action(action, phase, msg.delta_x, msg.delta_y);
            }
        }
    });

    // 5. Mantenemos el servidor UDP escuchando de forma asíncrona
    server.run().await
}

/// Función que genera un archivo PNG con el QR y lo abre usando `opener`
fn show_qr_popup(ip: &str, port: u16, pin: u32) {
    // 1. Preparamos los datos con el formato IP:PUERTO:PIN
    let qr_data = format!("{}:{}:{}", ip, port, pin);

    // 2. Generamos la matriz del código QR
    let code = match QrCode::new(qr_data.as_bytes()) {
        Ok(c) => c,
        Err(e) => {
            error!("Error al generar la matriz del QR: {}", e);
            return;
        }
    };

    // 3. Renderizamos el QR a una imagen en blanco y negro (Luma)
    let image = code
        .render::<Rgb<u8>>()
        .module_dimensions(8, 8)
        .dark_color(Rgb([0, 0, 0]))
        .light_color(Rgb([255, 255, 255]))
        .quiet_zone(true)
        .build();

    // 4. Definimos la ruta de guardado en la carpeta temporal de tu sistema operativo
    let mut path = env::temp_dir();
    path.push("omnipresent_qr_login.png");

    // 5. Guardamos la imagen en el disco
    if let Err(e) = image.save(&path) {
        error!(
            "Error al guardar la imagen del QR en {}: {}",
            path.display(),
            e
        );
        return;
    }

    info!("Generado código QR temporal en: {}", path.display());

    // 6. Abrimos la imagen con el visor predeterminado del sistema usando tu librería `opener`
    if let Err(e) = opener::open(&path) {
        error!("No se pudo abrir la imagen emergente del QR: {}", e);
        info!(
            "Pero puedes abrirla manualmente navegando a: {}",
            path.display()
        );
    } else {
        info!("¡Ventana emergente del QR abierta con éxito!");
    }
}
