use crate::mouse::strategy::MouseStrategy;
use log::info;

#[cfg(target_os = "linux")]
use crate::mouse::linux::LinuxMouseStrategy;

#[cfg(target_os = "windows")]
use crate::mouse::windows::WindowsMouseStrategy;

#[cfg(target_os = "macos")]
use crate::mouse::macos::MacOsMouseStrategy;

pub struct MouseStrategyFactory;

impl MouseStrategyFactory {
    pub fn create() -> Box<dyn MouseStrategy> {
        #[cfg(target_os = "linux")]
        {
            info!("Running on Linux. Using LinuxMouseStrategy.");
            Box::new(LinuxMouseStrategy::new())
        }

        #[cfg(target_os = "windows")]
        {
            info!("Running on Windows. Using WindowsMouseStrategy.");
            Box::new(WindowsMouseStrategy::new())
        }

        #[cfg(target_os = "macos")]
        {
            info!("Running on macOS. Using MacOsMouseStrategy.");
            Box::new(MacOsMouseStrategy::new())
        }

        // Fallback if compiled on an unsupported operating system (for example FreeBSD or Android)
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            panic!("Unsupported operating system.");
        }
    }
}
