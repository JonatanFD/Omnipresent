//! The local IPC channel between the `omni` CLI and the daemon, abstracted over
//! the platform's native local-IPC primitive:
//!
//! - **Unix**: a Unix-domain socket file in the config directory, mode `0600`
//!   so only the owner can command the daemon.
//! - **Windows**: a named pipe whose name is derived from the config directory,
//!   carrying a DACL that grants only the user the daemon runs as — the same
//!   promise as `0600`. It also rejects remote (network) clients and claims the
//!   first instance, so another process cannot squat the name.
//!
//! The server side (`IpcListener`) is async, driven by the daemon's Tokio
//! runtime. The client side (`connect_blocking`) is synchronous, for the CLI,
//! which is a thin one-shot request/response tool with no runtime of its own.
//! Both client handle types implement [`std::io::Read`] and [`std::io::Write`].

use crate::config::Paths;
use std::io;

#[cfg(unix)]
mod imp {
    use super::*;
    use std::path::PathBuf;
    use tokio::net::{UnixListener, UnixStream};

    /// One accepted CLI connection, as the daemon sees it.
    pub type IpcStream = UnixStream;
    /// A synchronous client handle for the CLI.
    pub type IpcClient = std::os::unix::net::UnixStream;

    /// The daemon's IPC listener: a Unix-domain socket, owner-only.
    pub struct IpcListener {
        inner: UnixListener,
        path: PathBuf,
    }

    impl IpcListener {
        pub fn bind(paths: &Paths) -> io::Result<Self> {
            use std::os::unix::fs::PermissionsExt;
            let path = paths.socket_file();
            let _ = std::fs::remove_file(&path);
            let inner = UnixListener::bind(&path)?;
            // Only the owner may command the daemon.
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            Ok(Self { inner, path })
        }

        pub async fn accept(&mut self) -> io::Result<IpcStream> {
            let (stream, _) = self.inner.accept().await?;
            Ok(stream)
        }
    }

    impl Drop for IpcListener {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// Connects to the running daemon, or fails if it is not listening.
    pub fn connect_blocking(paths: &Paths) -> io::Result<IpcClient> {
        std::os::unix::net::UnixStream::connect(paths.socket_file())
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::pipe_security::OwnerOnly;
    use std::time::{Duration, Instant};
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    /// How long a client keeps trying to connect while every pipe instance is
    /// momentarily taken.
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
    /// How long to wait between those attempts.
    const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(20);

    /// One accepted CLI connection, as the daemon sees it.
    pub type IpcStream = NamedPipeServer;
    /// A synchronous client handle for the CLI. A named pipe opened as a file
    /// behaves as a bidirectional byte stream.
    pub type IpcClient = std::fs::File;

    /// The daemon's IPC listener: a named pipe with one instance always waiting
    /// to be connected.
    pub struct IpcListener {
        /// The next instance, already created and waiting for a client.
        next: NamedPipeServer,
        name: String,
        /// Kept alive because every instance is created against it.
        security: OwnerOnly,
    }

    impl IpcListener {
        pub fn bind(paths: &Paths) -> io::Result<Self> {
            let name = paths.pipe_name();
            let mut security = OwnerOnly::new()?;
            let next = Self::create(&name, &mut security, true)?;
            Ok(Self {
                next,
                name,
                security,
            })
        }

        pub async fn accept(&mut self) -> io::Result<IpcStream> {
            // Wait for a client to connect to the waiting instance.
            self.next.connect().await?;
            // Stand up a fresh instance for the next client, and hand back the
            // one that just connected.
            let server = Self::create(&self.name, &mut self.security, false)?;
            Ok(std::mem::replace(&mut self.next, server))
        }

        /// Creates one pipe instance, owner-only and local-clients-only.
        fn create(
            name: &str,
            security: &mut OwnerOnly,
            first: bool,
        ) -> io::Result<NamedPipeServer> {
            let mut options = ServerOptions::new();
            options.reject_remote_clients(true);
            if first {
                // Claiming the first instance stops another process squatting
                // the name before the daemon gets there.
                options.first_pipe_instance(true);
            }
            // Safety: the attributes point at a descriptor owned by `security`,
            // which outlives this call.
            unsafe { options.create_with_security_attributes_raw(name, security.as_ptr()) }
        }
    }

    /// Connects to the running daemon, or fails if it is not listening.
    ///
    /// The daemon keeps exactly one instance waiting and creates the next only
    /// after a client takes it, so two clients arriving together can find the
    /// pipe busy for a moment. That is not "the daemon is not running", so a busy
    /// pipe is retried briefly rather than reported as an absent daemon.
    pub fn connect_blocking(paths: &Paths) -> io::Result<IpcClient> {
        let name = paths.pipe_name();
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        loop {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&name)
            {
                Ok(client) => return Ok(client),
                Err(e) if is_busy(&e) && Instant::now() < deadline => {
                    std::thread::sleep(CONNECT_RETRY_DELAY);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Whether the error means "every instance is taken right now", as opposed to
    /// there being no pipe at all.
    fn is_busy(error: &io::Error) -> bool {
        const ERROR_PIPE_BUSY: i32 = 231;
        error.raw_os_error() == Some(ERROR_PIPE_BUSY)
    }
}

pub use imp::{IpcClient, IpcListener, IpcStream, connect_blocking};
