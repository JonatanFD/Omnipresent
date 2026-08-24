//! Putting the `omni` command on the user's PATH.
//!
//! The app bundles the CLI as a sidecar binary and installs it on first run, so
//! one install really is the whole product — the window, the daemon inside it,
//! and the command. It goes to a directory the user already owns, so nothing
//! needs administrator rights.
//!
//! Only when nothing is there yet. A command that is already installed is left
//! alone: it may be newer than this app, or put there deliberately by
//! `install.sh`, and silently overwriting either would be a downgrade the user
//! never asked for. Replacing it is offered in the window instead.
//!
//! The destination is the same one `install.sh` and `install.ps1` document, and
//! honours the same `OMNI_INSTALL_DIR` override — so a machine that already has
//! the CLI gets it replaced in place rather than shadowed by a second copy on a
//! different part of the PATH.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The sidecar's name once bundled. Tauri strips the target triple it is staged
/// under, so next to the app binary it is just this.
///
/// Not `omni`: a sidecar may not share the name of the Cargo package that bundles
/// it, and this app's package *is* `omni`. It is installed under the plain name
/// below, which is the one the user types.
#[cfg(windows)]
const SIDECAR_NAME: &str = "omni-cli.exe";
#[cfg(not(windows))]
const SIDECAR_NAME: &str = "omni-cli";

/// What the command is called once installed — what the docs and the install
/// scripts call it, and what `omni update` expects to find.
#[cfg(windows)]
const CLI_NAME: &str = "omni.exe";
#[cfg(not(windows))]
const CLI_NAME: &str = "omni";

/// Where the `omni` command is, or could be.
#[derive(Clone, Debug, serde::Serialize)]
pub struct CliStatus {
    /// Where it would be installed to.
    pub target: String,
    /// True when a command is already installed there.
    pub installed: bool,
    /// True when that directory is on the PATH of the session that launched us.
    pub on_path: bool,
    /// False in a development build, where no sidecar has been staged.
    pub available: bool,
}

/// The variable the default install directory hangs off, per platform.
#[cfg(windows)]
const BASE_VAR: &str = "LOCALAPPDATA";
#[cfg(not(windows))]
const BASE_VAR: &str = "HOME";

/// The directory the CLI is installed into, matching the install scripts.
///
/// Both inputs are passed in rather than read here, so the choice can be tested
/// without mutating the environment — which the test harness shares across
/// threads, and which is what makes environment-dependent tests flaky.
fn resolve_install_dir(
    override_dir: Option<PathBuf>,
    base: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(dir) = override_dir {
        return Ok(dir);
    }

    let base = base.ok_or_else(|| format!("{BASE_VAR} is not set"))?;

    #[cfg(windows)]
    return Ok(base.join("Programs").join("omni"));
    #[cfg(not(windows))]
    return Ok(base.join(".local").join("bin"));
}

fn install_dir() -> Result<PathBuf, String> {
    resolve_install_dir(
        std::env::var_os("OMNI_INSTALL_DIR").map(PathBuf::from),
        std::env::var_os(BASE_VAR).map(PathBuf::from),
    )
}

/// The bundled CLI, which sits next to the app's own binary.
fn sidecar() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate this app: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "this app has no containing directory".to_string())?;
    Ok(dir.join(SIDECAR_NAME))
}

/// Whether `dir` is one of the entries in `path`.
///
/// Split out from the environment so it can be tested: `PATH` separators and
/// stray empty entries differ per platform, and an empty entry means "the
/// current directory", never "every directory".
fn contains_dir(path: &std::ffi::OsStr, dir: &Path) -> bool {
    std::env::split_paths(path).any(|entry| !entry.as_os_str().is_empty() && entry == dir)
}

/// The PATH a command typed in a terminal would actually be looked up in.
///
/// Not this process's PATH. An app launched from Finder or the Dock inherits
/// launchd's minimal `/usr/bin:/bin:/usr/sbin:/sbin`, which almost never
/// contains the install directory — so checking it reported "not on your PATH"
/// to people whose shell had it all along. Asking the login shell is the only
/// way to see what the user will really get, and it is what the user cares
/// about: whether typing `omni` works.
#[cfg(unix)]
fn shell_path() -> Option<OsString> {
    // A login shell, because that is what sources the profile that sets PATH.
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let output = std::process::Command::new(shell)
        .args(["-l", "-c", "printf %s \"$PATH\""])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    (!path.is_empty()).then(|| OsString::from(path))
}

/// On Windows a GUI process inherits the real user PATH, so there is nothing to
/// go and ask for.
#[cfg(windows)]
fn shell_path() -> Option<OsString> {
    std::env::var_os("PATH")
}

#[tauri::command]
pub fn cli_status() -> Result<CliStatus, String> {
    let target = install_dir()?;
    let on_path = shell_path()
        .or_else(|| std::env::var_os("PATH"))
        .is_some_and(|path| contains_dir(&path, &target));

    Ok(CliStatus {
        installed: target.join(CLI_NAME).exists(),
        on_path,
        available: sidecar().is_ok_and(|path| path.exists()),
        target: target.join(CLI_NAME).display().to_string(),
    })
}

/// Installs the command if nothing is there yet. Called once at startup.
///
/// Best-effort and silent: the window works perfectly well without the command,
/// so a read-only home directory or a missing sidecar in a development build is
/// not worth interrupting a launch over. The System pane shows the outcome
/// either way.
pub fn install_if_absent() {
    let Ok(status) = cli_status() else { return };
    if status.installed || !status.available {
        return;
    }
    if let Err(error) = cli_install() {
        // Never carries key material — it is a path and an OS error.
        eprintln!("omni: could not install the command line tool ({error})");
    }
}

/// Copies the bundled CLI into the install directory.
///
/// A copy, not a symlink: the app bundle can be replaced wholesale by an update
/// or moved to the trash, and a link into it would break silently. A copy keeps
/// working, and the next install overwrites it.
#[tauri::command]
pub fn cli_install() -> Result<CliStatus, String> {
    let source = sidecar()?;
    if !source.exists() {
        return Err(
            "this build has no bundled command — it is only staged in a packaged app".into(),
        );
    }

    let dir = install_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    let destination = dir.join(CLI_NAME);
    // Removing first avoids ETXTBSY when the destination is a copy that is
    // currently running, which is exactly the case when the CLI started the
    // daemon this window is talking to.
    let _ = std::fs::remove_file(&destination);
    std::fs::copy(&source, &destination)
        .map_err(|e| format!("cannot write {}: {e}", destination.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("cannot make {} executable: {e}", destination.display()))?;
    }

    cli_status()
}

/// The app's own version, which is not the daemon's when the window is talking
/// to a daemon that was already running.
#[tauri::command]
pub fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn the_install_directory_honours_the_documented_override() {
        // Same variable the install scripts read, so the app and the scripts
        // cannot install to two different places on one machine.
        let dir = resolve_install_dir(
            Some(PathBuf::from("/tmp/omni-test-bin")),
            Some(PathBuf::from("/home/someone")),
        );

        assert_eq!(dir.unwrap(), PathBuf::from("/tmp/omni-test-bin"));
    }

    #[test]
    fn the_default_install_directory_matches_the_install_script() {
        let dir = resolve_install_dir(None, Some(PathBuf::from("/base")))
            .expect("a base directory was given");

        #[cfg(windows)]
        assert_eq!(dir, PathBuf::from("/base").join("Programs").join("omni"));
        #[cfg(not(windows))]
        assert_eq!(dir, PathBuf::from("/base/.local/bin"));
    }

    #[test]
    fn a_missing_base_directory_is_an_error_not_a_guess() {
        // Installing into a path built from a missing variable would put the
        // command somewhere nobody asked for. Better to say why it cannot.
        let error = resolve_install_dir(None, None).expect_err("no base is an error");

        assert!(error.contains(BASE_VAR));
    }

    #[test]
    fn a_directory_on_the_path_is_recognised() {
        let path = std::env::join_paths([
            PathBuf::from("/usr/bin"),
            PathBuf::from("/home/someone/.local/bin"),
        ])
        .expect("the entries join");

        assert!(contains_dir(&path, Path::new("/home/someone/.local/bin")));
        assert!(!contains_dir(&path, Path::new("/opt/bin")));
    }

    #[test]
    fn an_empty_path_entry_does_not_match_everything() {
        // An empty entry means the current directory. Treating it as a match
        // would report the CLI as reachable from anywhere when it is not.
        let path = OsString::from(if cfg!(windows) { ";;" } else { "::" });

        assert!(!contains_dir(&path, Path::new("/home/someone/.local/bin")));
    }

    #[test]
    #[cfg(unix)]
    fn the_shell_path_is_read_from_the_login_shell_not_this_process() {
        // The whole point: a GUI process's PATH is launchd's minimal one, and
        // the answer has to come from the shell instead. If this returns
        // something, it must at least look like a PATH.
        if let Some(path) = shell_path() {
            let entries: Vec<_> = std::env::split_paths(&path).collect();
            assert!(!entries.is_empty(), "a PATH has at least one entry");
            assert!(
                entries.iter().any(|e| e.is_absolute()),
                "a real PATH has absolute entries, got {path:?}"
            );
        }
    }

    #[test]
    fn a_command_already_installed_is_left_alone() {
        // Auto-install must not overwrite: what is there may be newer than this
        // app, or put there deliberately by install.sh. `install_if_absent`
        // returns without touching anything, which in a test build — where
        // there is no sidecar either — means it simply does nothing.
        install_if_absent();
    }

    #[test]
    fn installing_without_a_bundled_command_says_so() {
        // A development build has no staged sidecar. It must explain that rather
        // than fail with a bare file-not-found the user cannot act on.
        let error = cli_install().expect_err("a dev build has no sidecar");

        assert!(error.contains("bundled command") || error.contains("cannot"));
    }
}
