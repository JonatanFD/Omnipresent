use log::{info, warn};
use rand::RngExt;
use std::fs;
use std::path::Path;

/// Name of the file where the PIN is stored.
const AUTH_FILE: &str = "omnipresent_auth.txt";

pub struct AuthInfo;

impl AuthInfo {
    /// Gets the saved token or generates a new one if it does not exist.
    /// Also allows forcing regeneration if `force_reset` is true.
    pub fn get_or_create_token(force_reset: bool) -> u32 {
        let path = Path::new(AUTH_FILE);

        // If we are not forcing a reset and the file exists, try to read it.
        if !force_reset && path.exists() {
            match fs::read_to_string(path) {
                Ok(contents) => {
                    // Trim whitespace and convert to a number.
                    match contents.trim().parse::<u32>() {
                        Ok(token) => {
                            info!("Security PIN loaded: {}", token);
                            return token;
                        }
                        Err(_) => warn!("Token file is invalid. Generating a new token."),
                    }
                }
                Err(e) => warn!("Could not read token file ({}). Generating a new token.", e),
            }
        }

        // If we reach this point, a new PIN is required (file missing, failed, or reset forced).
        let new_token = Self::generate_random_pin();

        // Persist the new PIN to disk for future runs.
        match fs::write(path, new_token.to_string()) {
            Ok(_) => info!(
                "New PIN generated and stored in '{}': {}",
                AUTH_FILE, new_token
            ),
            Err(e) => warn!(
                "New PIN {} generated but could not be stored on disk: {}",
                new_token, e
            ),
        }

        new_token
    }

    /// Generates a random 6-digit number (100000 - 999999).
    fn generate_random_pin() -> u32 {
        let mut rng = rand::rng();
        rng.random_range(100_000..=999_999)
    }
}
