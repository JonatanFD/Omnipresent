//! Injection: synthesizing events with `SendInput`, as if they came from real
//! hardware. Used while this machine is the Target.
//!
//! Every injected event is stamped with our marker in `dwExtraInfo` and arrives
//! flagged "injected", so the capture hooks skip it and never echo our own
//! output back onto the wire.
//!
//! Windows has no way to say "this keystroke is Ctrl-held"; a chord only exists
//! because the modifier's own key-down arrived first and has not come up. Input
//! events travel on unreliable datagrams, so one of those can go missing — and
//! then a chord is wrong, or worse, a lost key-up leaves a modifier stuck down
//! for good. Every key event carries the modifiers that were held when it was
//! captured, so the sink compares them against what it has actually injected and
//! makes up the difference before pressing the key. macOS gets this for free by
//! stamping the flags on each event.

use super::{PIXELS_PER_WHEEL_CLICK, WindowsInputError, keymap};
use crate::port::InputSink;
use omni_protocol::InputEvent;
use omni_protocol::input::{Action, Modifiers, MouseButton, ScrollDelta};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT,
    SendInput,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{SetCursorPos, WHEEL_DELTA, XBUTTON1, XBUTTON2};

/// Marker written to `dwExtraInfo` on everything we inject, so the capture
/// hooks can recognise and skip our own synthetic events.
pub(super) const INJECTED_MARKER: usize = 0x4F4D_4E49; // "OMNI"

/// The virtual key used to synthesize each modifier. The left-hand key stands in
/// for the modifier as a whole: when only the *effect* is known (a key event says
/// "Control was held") either side does, and the left one is the conventional
/// choice.
const MODIFIER_KEYS: [(Modifiers, u16); 4] = [
    (Modifiers::SHIFT, 0xA0),   // VK_LSHIFT
    (Modifiers::CONTROL, 0xA2), // VK_LCONTROL
    (Modifiers::ALT, 0xA4),     // VK_LMENU
    (Modifiers::META, 0x5B),    // VK_LWIN
];

/// One modifier key to press or release so the OS matches what the controller had
/// held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ModifierFix {
    pub modifier: Modifiers,
    pub action: Action,
}

/// The modifier keys to press and release so `held` becomes `wanted`.
///
/// Pure, so the repair itself can be tested without touching the OS. Releases
/// come before presses: a stuck modifier is the more disruptive of the two, so it
/// is cleared first.
pub(super) fn reconcile(held: Modifiers, wanted: Modifiers) -> Vec<ModifierFix> {
    let mut fixes = Vec::new();
    for modifier in Modifiers::ALL {
        let is_held = held.contains(modifier);
        let should_hold = wanted.contains(modifier);
        if is_held && !should_hold {
            fixes.push(ModifierFix {
                modifier,
                action: Action::Release,
            });
        }
    }
    for modifier in Modifiers::ALL {
        if wanted.contains(modifier) && !held.contains(modifier) {
            fixes.push(ModifierFix {
                modifier,
                action: Action::Press,
            });
        }
    }
    fixes
}

/// Injects remote input into the local OS. The production `InputSink`.
#[derive(Debug, Default)]
pub struct WindowsSink {
    /// Sub-notch scroll remainders, so small pixel deltas accumulate into whole
    /// wheel notches instead of vanishing.
    scroll_rem_x: i32,
    scroll_rem_y: i32,
    /// The modifiers this sink believes are down, from what it has injected. Kept
    /// in step with the controller's view so a lost datagram cannot strand one.
    held: Modifiers,
}

impl WindowsSink {
    /// Fallible for parity with the other platforms; building the sink on
    /// Windows cannot fail.
    pub fn new() -> Result<Self, WindowsInputError> {
        Ok(Self::default())
    }

    /// Presses or releases whatever it takes for the OS to agree with `wanted`
    /// before the key it applies to is injected.
    fn apply_modifiers(&mut self, wanted: Modifiers) -> Result<(), WindowsInputError> {
        for fix in reconcile(self.held, wanted) {
            let Some(&(_, vk)) = MODIFIER_KEYS.iter().find(|(m, _)| *m == fix.modifier) else {
                continue;
            };
            self.send_key(vk, false, fix.action)?;
            self.held = match fix.action {
                Action::Press => self.held.with(fix.modifier),
                Action::Release => self.held.without(fix.modifier),
            };
        }
        Ok(())
    }

    /// Injects one key, first making the OS agree about which modifiers are held.
    fn inject_key(
        &mut self,
        code: omni_protocol::KeyCode,
        action: Action,
        modifiers: Modifiers,
    ) -> Result<(), WindowsInputError> {
        match code.modifier() {
            // The event *is* a modifier change: obey it and record the result,
            // rather than repairing towards a snapshot that predates it.
            Some(modifier) => {
                self.held = match action {
                    Action::Press => self.held.with(modifier),
                    Action::Release => self.held.without(modifier),
                };
            }
            // An ordinary key: the modifiers it carries are the truth about what
            // the user was holding, so close any gap before pressing it.
            None => self.apply_modifiers(modifiers)?,
        }
        let Some(key) = keymap::vk_from_hid(code) else {
            return Ok(()); // an unmapped key is dropped, never guessed
        };
        self.send_key(key.vk, key.extended, action)
    }

    /// Synthesizes one key press or release. The lowest level: no bookkeeping.
    fn send_key(&self, vk: u16, extended: bool, action: Action) -> Result<(), WindowsInputError> {
        let mut flags = 0;
        if action == Action::Release {
            flags |= KEYEVENTF_KEYUP;
        }
        if extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: INJECTED_MARKER,
                },
            },
        };
        send(&input)
    }

    fn inject_mouse(
        &self,
        flags: u32,
        data: i32,
        dx: i32,
        dy: i32,
    ) -> Result<(), WindowsInputError> {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: data as u32,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: INJECTED_MARKER,
                },
            },
        };
        send(&input)
    }

    fn inject_button(&self, button: MouseButton, action: Action) -> Result<(), WindowsInputError> {
        let (flags, data) = match (button, action) {
            (MouseButton::Left, Action::Press) => (MOUSEEVENTF_LEFTDOWN, 0),
            (MouseButton::Left, Action::Release) => (MOUSEEVENTF_LEFTUP, 0),
            (MouseButton::Right, Action::Press) => (MOUSEEVENTF_RIGHTDOWN, 0),
            (MouseButton::Right, Action::Release) => (MOUSEEVENTF_RIGHTUP, 0),
            (MouseButton::Middle, Action::Press) => (MOUSEEVENTF_MIDDLEDOWN, 0),
            (MouseButton::Middle, Action::Release) => (MOUSEEVENTF_MIDDLEUP, 0),
            (MouseButton::Back, Action::Press) => (MOUSEEVENTF_XDOWN, XBUTTON1 as i32),
            (MouseButton::Back, Action::Release) => (MOUSEEVENTF_XUP, XBUTTON1 as i32),
            (MouseButton::Forward, Action::Press) => (MOUSEEVENTF_XDOWN, XBUTTON2 as i32),
            (MouseButton::Forward, Action::Release) => (MOUSEEVENTF_XUP, XBUTTON2 as i32),
            (MouseButton::Other(_), _) => return Ok(()), // unknown button: dropped
        };
        self.inject_mouse(flags, data, 0, 0)
    }

    fn inject_scroll(&mut self, delta: ScrollDelta) -> Result<(), WindowsInputError> {
        self.scroll_rem_x += delta.dx;
        self.scroll_rem_y += delta.dy;
        let notches_x = self.scroll_rem_x / PIXELS_PER_WHEEL_CLICK;
        let notches_y = self.scroll_rem_y / PIXELS_PER_WHEEL_CLICK;
        self.scroll_rem_x -= notches_x * PIXELS_PER_WHEEL_CLICK;
        self.scroll_rem_y -= notches_y * PIXELS_PER_WHEEL_CLICK;
        if notches_y != 0 {
            self.inject_mouse(MOUSEEVENTF_WHEEL, notches_y * WHEEL_DELTA as i32, 0, 0)?;
        }
        if notches_x != 0 {
            self.inject_mouse(MOUSEEVENTF_HWHEEL, notches_x * WHEEL_DELTA as i32, 0, 0)?;
        }
        Ok(())
    }
}

impl InputSink for WindowsSink {
    type Error = WindowsInputError;

    fn inject(&mut self, event: InputEvent) -> Result<(), Self::Error> {
        match event {
            InputEvent::Key {
                code,
                action,
                modifiers,
            } => self.inject_key(code, action, modifiers),
            InputEvent::Motion(delta) => self.inject_mouse(MOUSEEVENTF_MOVE, 0, delta.dx, delta.dy),
            // Absolute placement from a remote controller. SetCursorPos drives a
            // drag as well as a move (a held button persists) and arrives
            // flagged "injected", so the capture hook skips it.
            InputEvent::Pointer { x, y } => self.warp(x, y),
            InputEvent::Button { button, action } => self.inject_button(button, action),
            InputEvent::Scroll(delta) => self.inject_scroll(delta),
        }
    }

    fn warp(&mut self, x: i32, y: i32) -> Result<(), Self::Error> {
        // Absolute placement on an edge crossing. The position is in desktop
        // space, so it needs the desktop's origin added before the OS will
        // understand it — they differ whenever a monitor sits above or left of
        // the primary one. SetCursorPos arrives flagged "injected", so the
        // capture hook skips it.
        let (x, y) = match super::desktop_bounds() {
            Some(bounds) => bounds.to_screen(x, y),
            None => (x, y),
        };
        if unsafe { SetCursorPos(x, y) } == 0 {
            return Err(WindowsInputError::Injection);
        }
        Ok(())
    }
}

/// Sends one synthesized event; fails if the OS accepted none (e.g. the input
/// was blocked by a more-privileged window — see the elevation note in
/// `diagnose`).
fn send(input: &INPUT) -> Result<(), WindowsInputError> {
    let sent = unsafe { SendInput(1, input, std::mem::size_of::<INPUT>() as i32) };
    if sent == 1 {
        Ok(())
    } else {
        Err(WindowsInputError::Injection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(modifier: Modifiers) -> ModifierFix {
        ModifierFix {
            modifier,
            action: Action::Press,
        }
    }

    fn release(modifier: Modifiers) -> ModifierFix {
        ModifierFix {
            modifier,
            action: Action::Release,
        }
    }

    #[test]
    fn nothing_to_do_when_the_two_already_agree() {
        assert!(reconcile(Modifiers::NONE, Modifiers::NONE).is_empty());
        let ctrl = Modifiers::CONTROL;
        assert!(reconcile(ctrl, ctrl).is_empty());
    }

    #[test]
    fn a_missing_modifier_is_pressed_before_the_key() {
        // The controller had Control down but its key-down never arrived, so
        // Ctrl+C would have landed as a bare C.
        assert_eq!(
            reconcile(Modifiers::NONE, Modifiers::CONTROL),
            vec![press(Modifiers::CONTROL)]
        );
    }

    #[test]
    fn a_stuck_modifier_is_released() {
        // The key-up went missing, so the OS still thinks Control is down and
        // every later keystroke is a shortcut.
        assert_eq!(
            reconcile(Modifiers::CONTROL, Modifiers::NONE),
            vec![release(Modifiers::CONTROL)]
        );
    }

    #[test]
    fn releases_come_before_presses() {
        // Swapping one modifier for another: clear the stale one first, so the
        // two are never briefly held together as a different chord.
        let fixes = reconcile(Modifiers::ALT, Modifiers::CONTROL);

        assert_eq!(
            fixes,
            vec![release(Modifiers::ALT), press(Modifiers::CONTROL)]
        );
    }

    #[test]
    fn every_modifier_can_be_repaired_at_once() {
        let all = Modifiers::SHIFT
            .with(Modifiers::CONTROL)
            .with(Modifiers::ALT)
            .with(Modifiers::META);

        let fixes = reconcile(Modifiers::NONE, all);

        assert_eq!(fixes.len(), 4);
        assert!(fixes.iter().all(|f| f.action == Action::Press));
    }

    #[test]
    fn every_modifier_has_a_key_to_synthesize_it() {
        // reconcile can name any of the four, so the sink must be able to press
        // each one or a repair would be silently skipped.
        for modifier in Modifiers::ALL {
            assert!(
                MODIFIER_KEYS.iter().any(|(m, _)| *m == modifier),
                "no virtual key for a modifier reconcile can ask for"
            );
        }
    }
}
