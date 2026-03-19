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
            info!("Compilado para Linux. Instanciando LinuxMouseStrategy.");
            Box::new(LinuxMouseStrategy::new())
        }

        #[cfg(target_os = "windows")]
        {
            info!("Compilado para Windows. Instanciando WindowsMouseStrategy.");
            Box::new(WindowsMouseStrategy::new())
        }

        #[cfg(target_os = "macos")]
        {
            info!("Compilado para macOS. Instanciando MacOsMouseStrategy.");
            Box::new(MacOsMouseStrategy::new())
        }

        // Fallback si intentas compilar en algo raro como FreeBSD o Android
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            panic!("Sistema operativo no soportado nativamente.");
        }
    }
}
