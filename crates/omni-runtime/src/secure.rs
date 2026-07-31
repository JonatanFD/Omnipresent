//! Owner-only file writes for secret material (the TLS private key).
//!
//! The private key must never be readable by other users. Each platform has its
//! own access-control model, so this hides that behind one call:
//!
//! - **Unix**: the file is created with mode `0600` (owner read/write only).
//! - **Windows**: the file is created empty, its ACL is reset to inherit nothing
//!   and grant only the current user (`icacls`, best effort), and only then are
//!   the bytes written. The state directory already lives under the per-user
//!   profile, so this tightens an already user-scoped location rather than being
//!   the only barrier.
//!
//! Both order the work the same way — lock the file down, then write the secret
//! — so the key never exists on disk under looser permissions, however briefly.

use std::io;
use std::path::Path;

/// Writes `bytes` to `path`, readable and writable only by the current user.
///
/// Each platform tightens the file *before* the secret goes in, so there is no
/// window where the key exists with looser permissions.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_bytes(path, bytes)
}

#[cfg(unix)]
fn write_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

/// On Windows the file is created empty first and locked down before a single
/// secret byte is written, so the key is never briefly readable under whatever
/// permissions it would have inherited.
#[cfg(not(unix))]
fn write_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    restrict_to_owner(path);
    file.write_all(bytes)
}

/// Strips inherited permissions and grants only the current user. Unix already
/// did this through the file mode, so this is a no-op there.
#[cfg(unix)]
fn restrict_to_owner(_path: &Path) {}

/// On Windows, reset the file's ACL with `icacls` so it inherits nothing and
/// only the current user has access. Best effort: a failure leaves the file
/// readable per the (already per-user) parent directory, and is not fatal.
#[cfg(windows)]
fn restrict_to_owner(path: &Path) {
    let Ok(user) = std::env::var("USERNAME") else {
        return;
    };
    if user.is_empty() {
        return;
    }
    let _ = std::process::Command::new("icacls")
        .arg(path)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(format!("{user}:F"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("omni-test-secure-{name}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join("identity.key")
    }

    #[test]
    fn writes_the_bytes_back() {
        let path = temp_file("roundtrip");
        write_private(&path, b"secret-key-material").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"secret-key-material");
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn unix_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let path = temp_file("perms");
        write_private(&path, b"x").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_file(&path);
    }
}
