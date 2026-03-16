use std::env::consts::OS;

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
                println!("Detected OS: Linux. Instantiating LinuxMouseStrategy.");
                Box::new(LinuxMouseStrategy::new())
            }
            "windows" => {
                println!("Detected OS: Windows. Instantiating WindowsMouseStrategy.");
                Box::new(WindowsMouseStrategy::new())
            }
            "macos" => {
                println!("Detected OS: macOS. Instantiating MacOsMouseStrategy.");
                Box::new(MacOsMouseStrategy::new())
            }
            _ => {
                println!("OS not natively supported. Using Linux as fallback.");
                Box::new(LinuxMouseStrategy::new())
            }
        }
    }
}
