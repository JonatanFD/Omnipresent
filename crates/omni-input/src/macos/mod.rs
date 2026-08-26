//! macOS adapters for the input ports, over Core Graphics.
//!
//! - [`MacosSource`] captures keyboard and mouse events with a CGEvent tap and
//!   can *suppress* them (swallow them before the OS acts) while input is
//!   routed to a remote machine.
//! - [`MacosSink`] injects events with `CGEventPost`, as if they came from
//!   real hardware.
//!
//! Both require the Accessibility permission (System Settings → Privacy &
//! Security → Accessibility) — the least privilege macOS offers for this; the
//! daemon never runs as root.
//!
//! One key does not survive the round trip: **Caps Lock**. Capture works — the
//! source reads the latch from the event flags and sends a tap, so a Mac turning
//! Caps Lock on turns it on over on the other machine. Injection does not:
//! `CGEventPost` of the Caps Lock key moves no latch on macOS, which only the
//! IOKit HID interface can change. A remote machine's Caps Lock therefore has no
//! effect on a Mac being controlled.

mod convert;
pub mod keymap;
mod sink;
mod source;

pub use sink::MacosSink;
pub use source::MacosSource;

/// Platform-neutral aliases the Runtime wires against.
pub type OsSource = MacosSource;
pub type OsSink = MacosSink;

use crate::port::DesktopBounds;
use core_graphics::display::CGDisplay;
use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Why a macOS input operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacosInputError {
    /// The event tap could not be created. Almost always: the binary lacks the
    /// Accessibility permission.
    TapCreation,
    /// The capture thread is gone, so no more events will ever arrive.
    CaptureStopped,
    /// The OS refused to create or post an event.
    EventCreation,
}

impl std::fmt::Display for MacosInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MacosInputError::TapCreation => f.write_str(
                "could not create the event tap — grant this binary the Accessibility \
                 permission (System Settings → Privacy & Security → Accessibility)",
            ),
            MacosInputError::CaptureStopped => f.write_str("input capture stopped"),
            MacosInputError::EventCreation => f.write_str("could not synthesize an OS event"),
        }
    }
}

impl std::error::Error for MacosInputError {}

// From ApplicationServices/HIServices: whether this process may use the
// accessibility APIs — the permission gating both the event tap (capture)
// and CGEventPost (injection).
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    /// The same question, but able to show the system's own prompt. The
    /// options dictionary carries `AXTrustedCheckOptionPrompt`.
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> u8;
}

/// Asks for the Accessibility permission, showing the system prompt.
///
/// Checking and asking are different calls, and a window that knows the
/// permission is missing should put macOS's own dialog in front of the user
/// rather than describe a settings pane and leave them to find it.
///
/// Returns whether it is *already* granted. The answer to the prompt never
/// arrives here: macOS records it against the responsible app and the process
/// has to be restarted before a tap can be created, so the caller's job is to
/// ask and then tell the user to stop and start the daemon.
///
/// Harmless when already granted — macOS shows nothing — and it will not nag,
/// because the system suppresses a repeat prompt for a process that has been
/// answered once.
pub fn request_input_permission() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    // The key is exported as a CFStringRef symbol, but its value is documented
    // and stable, and naming it directly avoids another extern declaration.
    let prompt = CFString::from_static_string("AXTrustedCheckOptionPrompt");
    let options = CFDictionary::from_CFType_pairs(&[(prompt, CFBoolean::true_value())]);

    // Safety: the dictionary outlives the call, and the function only reads it.
    unsafe { AXIsProcessTrustedWithOptions(options.as_CFTypeRef() as *const _) != 0 }
}

/// Reports whether the OS permissions capture and injection need are granted.
///
/// The verdict is for *this process*. macOS attributes the Accessibility
/// permission to the responsible app — the terminal for a CLI run, the GUI app
/// when the GUI starts the daemon — so check it the same way you start it.
pub fn diagnose() -> Vec<crate::diag::Check> {
    use crate::diag::Check;
    let trusted = unsafe { AXIsProcessTrusted() } != 0;
    let check = if trusted {
        Check::ok(
            "accessibility permission",
            "granted — capture and injection available",
        )
    } else {
        Check::failed(
            "accessibility permission",
            "not granted — System Settings → Privacy & Security → Accessibility: \
             add whatever launches the daemon. macOS grants this to the *responsible* \
             app, which is the terminal when you run `omni start` yourself and \
             Omnipresent.app when you start it from the app. Granting it to one does \
             not grant it to the other. Rebuilding the binary revokes the grant; \
             remove and re-add it after a rebuild, then restart the daemon",
        )
    };
    vec![check]
}

/// Prepares the process before any capture or screen query. macOS reports
/// display geometry in a single, consistent coordinate space, so there is
/// nothing to do here; the hook exists only so the Runtime can call it on
/// every platform. (See the Windows adapter, where it declares DPI awareness.)
pub fn prepare_process() {}

/// Everything the desktop covers, across every attached display.
///
/// The union of the displays rather than just the main one: the cursor moves
/// over all of them, and treating only the main display as real made a cursor on
/// a second screen look pinned to the main one's edge — which reads as the user
/// pushing through to a peer.
pub fn desktop_bounds() -> Option<DesktopBounds> {
    let displays = CGDisplay::active_displays().ok()?;
    let mut union: Option<(f64, f64, f64, f64)> = None;
    for id in displays {
        let bounds = CGDisplay::new(id).bounds();
        let (left, top) = (bounds.origin.x, bounds.origin.y);
        let (right, bottom) = (left + bounds.size.width, top + bounds.size.height);
        union = Some(match union {
            None => (left, top, right, bottom),
            Some((l, t, r, b)) => (l.min(left), t.min(top), r.max(right), b.max(bottom)),
        });
    }
    // With no display list to go on, fall back to the main display alone.
    let (left, top, right, bottom) = union.unwrap_or_else(|| {
        let bounds = CGDisplay::main().bounds();
        (
            bounds.origin.x,
            bounds.origin.y,
            bounds.origin.x + bounds.size.width,
            bounds.origin.y + bounds.size.height,
        )
    });
    let (width, height) = ((right - left) as u32, (bottom - top) as u32);
    (width > 0 && height > 0).then(|| DesktopBounds::new(left as i32, top as i32, width, height))
}

/// The size of the desktop — the geometry Topology builds the virtual desktop
/// from.
pub fn primary_screen_size() -> Option<(u32, u32)> {
    desktop_bounds().map(DesktopBounds::size)
}

/// Where the cursor currently is, in desktop space (0-based, so a display placed
/// left of or above the main one does not report negative coordinates).
pub fn cursor_position() -> Option<(i32, i32)> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let location = CGEvent::new(source).ok()?.location();
    let bounds = desktop_bounds()?;
    Some(bounds.to_desktop(location.x as i32, location.y as i32))
}
