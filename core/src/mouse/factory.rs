use std::env::consts::OS;

use log::info;

use crate::mouse::{
    linux::LinuxMouseStrategy, macos::MacOsMouseStrategy, strategy::MouseStrategy,
    windows::WindowsMouseStrategy,
};

pub struct MouseStrategyFactory;

impl MouseStrategyFactory {
    // We return a Trait Object (Box<dyn Trait>)
    pub fn create() -> Box<dyn MouseStrategy> {
        match OS {
            "linux" => {
                info!("Detected OS: Linux. Instantiating LinuxMouseStrategy.");
                Box::new(LinuxMouseStrategy::new())
            }
            "windows" => {
                info!("Detected OS: Windows. Instantiating WindowsMouseStrategy.");
                Box::new(WindowsMouseStrategy::new())
            }
            "macos" => {
                info!("Detected OS: macOS. Instantiating MacOsMouseStrategy.");
                Box::new(MacOsMouseStrategy::new())
            }
            _ => {
                info!("OS not natively supported. Using Linux as fallback.");
                Box::new(LinuxMouseStrategy::new())
            }
        }
    }
}
