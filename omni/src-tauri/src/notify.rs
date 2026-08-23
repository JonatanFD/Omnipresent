//! Desktop notifications.
//!
//! Only one thing is worth interrupting the user for: a machine asking for
//! control of this one. It needs an answer, it expires on its own, and the
//! window is usually closed when it arrives. Everything else the app has to say
//! can wait for someone to open it.

use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

/// Tells the user a peer is waiting on their decision.
///
/// Best-effort: notifications can be refused by the OS or turned off by the
/// user, and the request is still answerable from the tray and the window when
/// they are. A failure here must never affect the session.
pub fn incoming_request<R: Runtime>(app: &AppHandle<R>, host: &str) {
    let _ = app
        .notification()
        .builder()
        .title("Omnipresent")
        .body(body(host))
        .show();
}

/// The wording. The fingerprint is deliberately left out: it is long, it does
/// not fit a notification, and approving on the strength of one glimpsed in a
/// popup is exactly the habit TOFU pinning is meant to discourage.
fn body(host: &str) -> String {
    format!("{host} is asking to control this machine. Accept from the menu bar or the window.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_names_the_machine_asking() {
        assert!(body("studio.local").starts_with("studio.local is asking"));
    }

    #[test]
    fn the_body_says_where_to_answer() {
        // A notification the user cannot act on is just noise, so it has to
        // point at the two places that can answer.
        let body = body("laptop");

        assert!(body.contains("menu bar"));
        assert!(body.contains("window"));
    }

    #[test]
    fn the_body_carries_no_fingerprint() {
        // Nothing here should invite trusting a key from a popup.
        let body = body("laptop");

        assert!(!body.contains("fingerprint"));
    }
}
