//! Asking the OS for the permission the daemon needs.
//!
//! Checking and asking are different calls, and the app was only ever checking.
//! `omni doctor` reports that Accessibility is missing, which is the right thing
//! for a terminal — but a window that knows the permission is missing should put
//! the system's own dialog in front of the user rather than describe a settings
//! pane and leave them to find it.

/// Asks macOS for the Accessibility permission, showing the system prompt.
///
/// Returns whether it is already granted. The prompt is asynchronous and the
/// answer never arrives here: macOS grants it to the *responsible* application
/// and requires a restart of the process before the tap can be created, so the
/// window's job is to ask and then tell the user to stop and start the daemon.
///
/// Only prompts once per app launch — macOS itself suppresses repeats — and
/// does nothing at all when the permission is already there.
#[tauri::command]
pub fn request_input_permission() -> bool {
    omni_runtime::request_input_permission()
}
