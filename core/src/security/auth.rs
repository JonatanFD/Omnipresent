use log::{info, warn};
use rand::{Rng, RngExt};
use std::fs;
use std::path::Path;

// El nombre del archivo donde guardaremos el PIN
const AUTH_FILE: &str = "omnipresent_auth.txt";

pub struct AuthInfo;

impl AuthInfo {
    /// Obtiene el token guardado o genera uno nuevo si no existe.
    /// También permite forzar la regeneración si `force_reset` es true.
    pub fn get_or_create_token(force_reset: bool) -> u32 {
        let path = Path::new(AUTH_FILE);

        // Si no estamos forzando un reinicio y el archivo existe, intentamos leerlo
        if !force_reset && path.exists() {
            match fs::read_to_string(path) {
                Ok(contents) => {
                    // Limpiamos espacios/saltos de línea y convertimos a número
                    match contents.trim().parse::<u32>() {
                        Ok(token) => {
                            info!("Pin de seguridad cargado exitosamente: {}", token);
                            return token;
                        }
                        Err(_) => warn!("El archivo de token es inválido. Generando uno nuevo..."),
                    }
                }
                Err(e) => warn!(
                    "No se pudo leer el archivo de token ({}). Generando uno nuevo...",
                    e
                ),
            }
        }

        // Si llegamos aquí, necesitamos un PIN nuevo (no existía, falló o se forzó el reset)
        let new_token = Self::generate_random_pin();

        // Guardamos el nuevo PIN en el archivo para la próxima vez
        match fs::write(path, new_token.to_string()) {
            Ok(_) => info!(
                "Nuevo PIN generado y guardado en '{}': {}",
                AUTH_FILE, new_token
            ),
            Err(e) => warn!(
                "Se generó el PIN {}, pero no se pudo guardar en disco: {}",
                new_token, e
            ),
        }

        new_token
    }

    /// Genera un número aleatorio de 6 dígitos (100000 - 999999)
    fn generate_random_pin() -> u32 {
        let mut rng = rand::rng();
        rng.random_range(100_000..=999_999)
    }
}
