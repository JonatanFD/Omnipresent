use log::{info, warn};
use rand::{Rng, RngExt};
use std::fs;
use std::path::Path;

// The name of the file where we will save the PIN
const AUTH_FILE: &str = "omnipresent_auth.txt";

pub struct AuthInfo;

impl AuthInfo {
    /// Gets the saved token or generates a new one if it does not exist.
    /// Also allows forcing regeneration if `force_reset` is true.
    pub fn get_or_create_token(force_reset: bool) -> u32 {
        let path = Path::new(AUTH_FILE);

        // If we are not forcing a reset and the file exists, we attempt to read it
        if !force_reset && path.exists() {
            match fs::read_to_string(path) {
                Ok(contents) => {
                    // Clean spaces/newlines and convert to a number
                    match contents.trim().parse::<u32>() {
                        Ok(token) => {
                            info!("Security pin successfully loaded: {}", token);
                            return token;
                        }
                        Err(_) => warn!("The token file is invalid. Generating a new one..."),
                    }
                }
                Err(e) => warn!(
                    "Could not read the token file ({}). Generating a new one...",
                    e
                ),
            }
        }

        // If we get here, we need a new PIN (did not exist, failed, or reset was forced)
        let new_token = Self::generate_random_pin();

        // We save the new PIN to the file for next time
        match fs::write(path, new_token.to_string()) {
            Ok(_) => info!(
                "New PIN generated and saved in '{}': {}",
                AUTH_FILE, new_token
            ),
            Err(e) => warn!(
                "PIN {} was generated, but could not be saved to disk: {}",
                new_token, e
            ),
        }

        new_token
    }

    /// Generates a random 6-digit number (100000 - 999999)
    fn generate_random_pin() -> u32 {
        let mut rng = rand::rng();
        rng.random_range(100_000..=999_999)
    }
}
