use image::Luma;
use local_ip_address::local_ip;
use log::{error, info};
use qrcode::QrCode;
use rand::RngExt;
use std::net::IpAddr;

pub struct AuthInfo {
    pub ip: IpAddr,
    pub port: u16,
    pub token: u32, // 6-digit PIN
}

impl AuthInfo {
    pub fn generate(port: u16) -> Self {
        // 1. Get the local IP address of the machine on the Wi-Fi/Ethernet network
        let ip = local_ip().expect("Failed to retrieve the machine's local IP address");

        // 2. Generate a random 6-digit token
        let token: u32 = rand::rng().random_range(100_000..=999_999);

        // 3. Create the payload string that Android will read
        // We use a custom URI scheme so it is easy to parse on the mobile app
        let payload = format!("omnipresent://{}:{}?token={}", ip, port, token);

        // 4. Generate the QR code in memory
        let code = QrCode::new(payload.as_bytes()).expect("Failed to generate the QR code");

        // 5. Save the QR code as a PNG image
        let image_path = "pairing_qr.png";
        let image = code.render::<Luma<u8>>().build();
        image.save(image_path).expect("Failed to save the PNG file");

        if let Err(e) = opener::open(image_path) {
            error!("Could not open the QR code image automatically: {}", e);
        }

        info!("📍 Server IP       : {}", ip);
        info!("🔌 Port            : {}", port);
        info!("🔑 Auth Token (PIN): {}", token);
        info!("🖼️  Image saved     : pairing_qr.png (in the project root)");

        Self { ip, port, token }
    }
}
