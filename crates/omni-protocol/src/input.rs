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

    /// Which modifier this key controls, or `None` if it is an ordinary key.
    ///
    /// HID gives the eight modifier keys the usages `0xE0`–`0xE7`, left hand
    /// then right. Both sides of a pair control the same modifier, so Right
    /// Shift and Left Shift both report [`Modifiers::SHIFT`].
    pub const fn modifier(self) -> Option<Modifiers> {
        match self.0 {
            0xE0 | 0xE4 => Some(Modifiers::CONTROL),
            0xE1 | 0xE5 => Some(Modifiers::SHIFT),
            0xE2 | 0xE6 => Some(Modifiers::ALT),
            0xE3 | 0xE7 => Some(Modifiers::META),
            _ => None,
        }
    }

    /// Whether this key is one of the eight modifier keys.
    pub const fn is_modifier(self) -> bool {
        self.modifier().is_some()
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

    /// Removes the modifiers in `other` from this set.
    pub const fn without(self, other: Modifiers) -> Self {
        Modifiers(self.0 & !other.0)
    }

    /// The modifiers in this set but not in `other` — what is missing over there.
    pub const fn difference(self, other: Modifiers) -> Self {
        Modifiers(self.0 & !other.0)
    }

    /// Whether no modifier at all is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The four modifiers, each on its own, for walking a set one at a time.
    pub const ALL: [Modifiers; 4] = [
        Modifiers::SHIFT,
        Modifiers::CONTROL,
        Modifiers::ALT,
        Modifiers::META,
    ];
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
/// scroll up. The unit is milli-lines — see [`MILLILINES_PER_LINE`].
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

/// How many [`ScrollDelta`] units one line of scrolling is worth. One notch of a
/// mouse wheel is one line.
///
/// A `ScrollDelta` is measured in **lines**, not pixels, and that choice decides
/// who picks the scrolling speed. A line is what every desktop OS lets its user
/// size: Windows has "lines to scroll per notch", macOS has a scrolling-speed
/// slider, and each application turns a line into pixels using its own row
/// height. So a machine sends *how far the wheel turned* and the machine
/// receiving it decides *how much that should scroll*, using its own settings —
/// which is what a user expects from their own computer.
///
/// Sending pixels instead put that decision on the sending machine, and the two
/// sinks disagreed about it: Windows converted back to notches and let Windows
/// scale them, while macOS injected the pixels as final, so macOS never applied
/// its own speed and a Mac scrolled a fraction of what it should.
///
/// The unit is a **thousandth** of a line so that continuous devices survive it.
/// A mouse wheel turns in whole notches, but a trackpad reports fractions of a
/// line, and a whole-number unit would round every small swipe away to nothing.
pub const MILLILINES_PER_LINE: i32 = 1_000;

// A zero or negative line would make every platform drop scrolling entirely, or
// scroll backwards. Checked here so it can never be set to one.
const _: () = assert!(MILLILINES_PER_LINE > 0);

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
    fn both_sides_of_a_modifier_pair_control_the_same_modifier() {
        // Left and right Shift are different keys but one modifier.
        assert_eq!(KeyCode::new(0xE1).modifier(), Some(Modifiers::SHIFT));
        assert_eq!(KeyCode::new(0xE5).modifier(), Some(Modifiers::SHIFT));
        assert_eq!(KeyCode::new(0xE0).modifier(), Some(Modifiers::CONTROL));
        assert_eq!(KeyCode::new(0xE4).modifier(), Some(Modifiers::CONTROL));
        assert_eq!(KeyCode::new(0xE2).modifier(), Some(Modifiers::ALT));
        assert_eq!(KeyCode::new(0xE6).modifier(), Some(Modifiers::ALT));
        assert_eq!(KeyCode::new(0xE3).modifier(), Some(Modifiers::META));
        assert_eq!(KeyCode::new(0xE7).modifier(), Some(Modifiers::META));
    }

    #[test]
    fn an_ordinary_key_controls_no_modifier() {
        assert_eq!(KeyCode::new(0x04).modifier(), None); // A
        assert!(!KeyCode::new(0x04).is_modifier());
        assert!(KeyCode::new(0xE1).is_modifier());
        // Caps Lock latches a state; it is not one of the eight modifier keys.
        assert!(!KeyCode::CAPS_LOCK.is_modifier());
    }

    #[test]
    fn modifier_sets_can_be_narrowed_and_compared() {
        let both = Modifiers::SHIFT.with(Modifiers::CONTROL);

        assert_eq!(both.without(Modifiers::SHIFT), Modifiers::CONTROL);
        assert_eq!(both.difference(Modifiers::SHIFT), Modifiers::CONTROL);
        assert!(Modifiers::NONE.is_empty());
        assert!(!both.is_empty());
        assert_eq!(Modifiers::ALL.len(), 4);
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
