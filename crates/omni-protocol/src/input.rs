//! Input events: the keyboard and mouse actions carried between machines.
//!
//! These types are platform-neutral on purpose. The Input module's per-OS
//! adapters translate between this vocabulary and native event codes, so nothing
//! here knows about macOS or Linux.

use serde::{Deserialize, Serialize};

/// A single keyboard or mouse event, captured on the Controller and injected on
/// the Target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEvent {
    /// A key was pressed or released.
    Key {
        code: KeyCode,
        action: Action,
        modifiers: Modifiers,
    },
    /// The mouse moved by a relative amount. Used between the OS and the
    /// virtual-desktop model; not what travels to a remote target.
    Motion(MouseDelta),
    /// The pointer is at this absolute position on the target's screen, in that
    /// screen's pixels. This is what the controller sends while driving a remote
    /// machine: the controller maps the cursor into the peer's screen using both
    /// machines' sizes (the virtual desktop), so the two cursors can never drift
    /// apart the way accumulated relative deltas would.
    Pointer { x: i32, y: i32 },
    /// A mouse button was pressed or released.
    Button { button: MouseButton, action: Action },
    /// The scroll wheel moved.
    Scroll(ScrollDelta),
}

/// Whether an input went down or came back up. Shared by keys and mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Press,
    Release,
}

/// A platform-neutral key identifier, using USB HID usage codes as the canonical
/// representation. Adapters map native scancodes to and from this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCode(u32);

impl KeyCode {
    /// Return on the main key block.
    pub const RETURN: KeyCode = KeyCode(0x28);

    /// Caps Lock. Unlike the other modifiers this one *latches*: pressing it
    /// changes a state that stays until it is pressed again, so adapters treat it
    /// as a tap rather than something held.
    pub const CAPS_LOCK: KeyCode = KeyCode(0x39);

    /// Enter on the numeric keypad, which HID reports separately from
    /// [`KeyCode::RETURN`]. Some applications tell the two apart.
    pub const KEYPAD_ENTER: KeyCode = KeyCode(0x58);

    /// Wraps a raw HID usage code.
    pub const fn new(code: u32) -> Self {
        Self(code)
    }

    /// The underlying code.
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// The set of modifier keys held down at the time of an event, packed into a
/// single byte so it stays cheap on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    pub const SHIFT: Modifiers = Modifiers(1 << 0);
    pub const CONTROL: Modifiers = Modifiers(1 << 1);
    pub const ALT: Modifiers = Modifiers(1 << 2);
    /// The platform "command"/"super"/"windows" key.
    pub const META: Modifiers = Modifiers(1 << 3);

    /// An empty modifier set.
    pub const fn empty() -> Self {
        Modifiers(0)
    }

    /// Returns whether every modifier in `other` is also set here.
    pub const fn contains(self, other: Modifiers) -> bool {
        self.0 & other.0 == other.0
    }

    /// Adds the modifiers in `other` to this set.
    pub const fn with(self, other: Modifiers) -> Self {
        Modifiers(self.0 | other.0)
    }
}

/// The mouse buttons we distinguish. `Other` carries any extra button by its
/// platform index so unusual mice still work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u8),
}

/// Relative mouse movement, in device pixels. Relative deltas (not absolute
/// positions) are what flow to the active Target while it is being controlled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MouseDelta {
    pub dx: i32,
    pub dy: i32,
}

impl MouseDelta {
    pub const fn new(dx: i32, dy: i32) -> Self {
        Self { dx, dy }
    }
}

/// Scroll wheel movement. `dx` is horizontal, `dy` vertical; positive `dy` is a
/// scroll up. The unit is scroll pixels — see [`PIXELS_PER_WHEEL_NOTCH`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ScrollDelta {
    pub dx: i32,
    pub dy: i32,
}

impl ScrollDelta {
    pub const fn new(dx: i32, dy: i32) -> Self {
        Self { dx, dy }
    }
}

/// How many [`ScrollDelta`] units one notch of a mouse wheel is worth.
///
/// A `ScrollDelta` is measured in scroll pixels, because that is the finest
/// thing any platform reports: macOS hands out continuous pixel deltas (a
/// trackpad produces many small ones), while Windows and Linux report whole
/// wheel notches. The notched platforms multiply by this constant on the way out
/// and divide on the way in, keeping the remainder so nothing is lost.
///
/// It lives here, in the shared vocabulary, because it defines what a unit on
/// the wire *means*. With each platform picking its own number, one notch on a
/// Mac and one notch on a PC scrolled visibly different amounts. The value is
/// matched to the delta macOS reports for a single notch, so a notch is a notch
/// in every direction; the notched platforms only have to agree with each other,
/// so they stay consistent whatever it is set to.
pub const PIXELS_PER_WHEEL_NOTCH: i32 = 10;

// A zero or negative notch would make the notched platforms drop every scroll,
// or scroll backwards. Checked here so it can never be set to one.
const _: () = assert!(PIXELS_PER_WHEEL_NOTCH > 0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_default_to_empty() {
        assert_eq!(Modifiers::default(), Modifiers::NONE);
        assert!(Modifiers::empty().contains(Modifiers::NONE));
    }

    #[test]
    fn modifiers_combine_and_report_membership() {
        let combo = Modifiers::CONTROL.with(Modifiers::SHIFT);

        assert!(combo.contains(Modifiers::CONTROL));
        assert!(combo.contains(Modifiers::SHIFT));
        assert!(combo.contains(Modifiers::CONTROL.with(Modifiers::SHIFT)));
        assert!(!combo.contains(Modifiers::ALT));
    }

    #[test]
    fn empty_modifiers_contain_only_nothing() {
        assert!(!Modifiers::NONE.contains(Modifiers::SHIFT));
        assert!(Modifiers::NONE.contains(Modifiers::NONE));
    }

    #[test]
    fn key_code_round_trips_its_value() {
        assert_eq!(KeyCode::new(0x04).value(), 0x04);
    }

    #[test]
    fn deltas_carry_their_components() {
        assert_eq!(MouseDelta::new(-3, 5), MouseDelta { dx: -3, dy: 5 });
        assert_eq!(ScrollDelta::new(0, -1), ScrollDelta { dx: 0, dy: -1 });
    }

    #[test]
    fn the_named_keys_are_their_hid_usages() {
        // Adapters on both sides of a session key off these, so the numbers are
        // part of the contract, not an implementation detail.
        assert_eq!(KeyCode::RETURN.value(), 0x28);
        assert_eq!(KeyCode::CAPS_LOCK.value(), 0x39);
        assert_eq!(KeyCode::KEYPAD_ENTER.value(), 0x58);
        // Keypad Enter is a different key from Return.
        assert_ne!(KeyCode::RETURN, KeyCode::KEYPAD_ENTER);
    }
}
