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

// Libraries for QR popup
use image::Rgb;
use local_ip_address::local_ip;
use qrcode::QrCode;

// Fixed server port
const SERVER_PORT: u16 = 9090;

#[tokio::main]
async fn main() -> io::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("Starting Omnipresent server");

    // Check CLI flags to reset PIN or show QR
    let args: Vec<String> = env::args().collect();
    let force_reset = args.contains(&String::from("--reset-pin"));
    let show_qr = args.contains(&String::from("--qr")) || args.contains(&String::from("-qr"));

    // Communication channel between UDP server and mouse controller
    let (tx, mut rx) = mpsc::channel::<TrackpadMessage>(100);

    // 1. Start server with fixed port
    let mut server = OmnipresentServer::bind(tx, SERVER_PORT).await?;

    // 2. Load saved PIN (or create a new one)
    let secure_pin = AuthInfo::get_or_create_token(force_reset);
    server.set_token(secure_pin);

    // 3. Start discovery service in a separate task
    let discovery_port = SERVER_PORT;
    let discovery_token = secure_pin;
    tokio::spawn(async move {
        if let Err(e) =
            OmnipresentServer::start_discovery_service(discovery_port, discovery_token).await
        {
            error!("Discovery service failed: {}", e);
        }
    });

    // 4. Get IP and optionally show QR as a popup window
    match local_ip() {
        Ok(my_local_ip) => {
            info!("Local IP detected: {}", my_local_ip);
            if show_qr {
                // Generate and open PNG image
                show_qr_popup(&my_local_ip.to_string(), SERVER_PORT, secure_pin);
            } else {
                info!("Run with --qr to display the pairing QR code");
            }
        }
        Err(e) => {
            error!("Could not get local IP for QR: {}", e);
            info!("Connect using port {} and PIN {}", SERVER_PORT, secure_pin);
        }
    }

    // 5. Start mouse controller thread
    std::thread::spawn(move || {
        let strategy = MouseStrategyFactory::create();
        let mut controller = InputController::new(strategy);

        info!("Input controller started and listening");

        while let Some(msg) = rx.blocking_recv() {
            let action = msg.action();
            let phase = msg.phase();

            // Pure movement
            if (msg.delta_x != 0.0 || msg.delta_y != 0.0) && action == ActionType::NoAction {
                controller.move_mouse(msg.delta_x, msg.delta_y);
            }

            // Actions (clicks, scrolls, swipes)
            if action != ActionType::NoAction {
                controller.execute_action(action, phase, msg.delta_x, msg.delta_y);
            }
        }
    });

    // 6. Keep UDP server listening asynchronously
    info!(
        "Server is ready. Listening on port {} with PIN {}",
        SERVER_PORT, secure_pin
    );
    server.run().await
}

/// Generates a PNG file with the QR code and opens it using `opener`.
fn show_qr_popup(ip: &str, port: u16, pin: u32) {
    // 1. Prepare data with omnipresent://IP:PORT/?token=PIN format
    let qr_data = format!("omnipresent://{}:{}/?token={}", ip, port, pin);

    // 2. Generate QR code matrix
    let code = match QrCode::new(qr_data.as_bytes()) {
        Ok(c) => c,
        Err(e) => {
            error!("Error generating QR matrix: {}", e);
            return;
        }
    };

    // 3. Render QR to a black and white image
    let image = code
        .render::<Rgb<u8>>()
        .module_dimensions(8, 8)
        .dark_color(Rgb([0, 0, 0]))
        .light_color(Rgb([255, 255, 255]))
        .quiet_zone(true)
        .build();

    // 4. Define save path in OS temp folder
    let mut path = env::temp_dir();
    path.push("omnipresent_qr_login.png");

    // 5. Save image to disk
    if let Err(e) = image.save(&path) {
        error!("Error saving QR image to {}: {}", path.display(), e);
        return;
    }

    info!("Temporary QR code generated at {}", path.display());

    // 6. Open image with system default viewer using `opener`
    if let Err(e) = opener::open(&path) {
        error!("Could not open QR image: {}", e);
        info!("Open the file manually at {}", path.display());
    } else {
        info!("QR popup window opened");
    }
}
