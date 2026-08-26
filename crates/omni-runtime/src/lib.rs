//! Runtime: the composition root and long-running daemon.
//!
//! Wires concrete adapters into every module's ports and drives the
//! capture → route → send and receive → inject pipelines. Exposes a local IPC
//! surface (a Unix socket in the config directory) so the `omni` CLI can issue
//! commands and receive answers. The only place that depends on every module;
//! nothing depends on Runtime except the CLI binary.

pub mod config;
pub mod daemon;
pub mod doctor;
pub mod identity;
pub mod ipc;
pub mod ipc_transport;
/// The named pipe's access control. Windows only: on Unix the socket's file
/// mode does the same job.
#[cfg(windows)]
pub mod pipe_security;
pub mod ratelimit;
pub mod secure;
pub mod trust;

pub use config::{Config, Paths};
pub use daemon::{DaemonError, run, run_with_paths};

/// Prepares the current process before it reads the screen or installs input
/// hooks: on Windows this declares per-monitor DPI awareness so every
/// coordinate API speaks real pixels (a no-op on other platforms). The CLI
/// calls it first thing so `omni doctor` and the daemon it launches agree on
/// the display geometry. Safe to call once per process; the daemon also calls
/// it itself, which is harmless.
pub fn prepare_process() {
    omni_input::platform::prepare_process();
}

/// Asks the OS for the permission capture and injection need, showing the
/// system's own prompt where there is one. Returns whether it is *already*
/// granted.
///
/// Only macOS has something to ask for. Windows needs no permission for
/// ordinary windows, and Linux is group membership and a udev rule — neither is
/// something a running process can prompt for, so both answer "granted" and
/// leave `doctor` to report what is actually missing.
pub fn request_input_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        omni_input::platform::request_input_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
