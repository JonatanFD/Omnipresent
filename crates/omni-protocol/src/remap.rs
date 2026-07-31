//! Relabelling modifier keys on the way to a peer.
//!
//! The same physical habit means different keys on different systems: on macOS
//! copy is Command-C, on Windows and Linux it is Control-C. Both send the key
//! faithfully — Command is HID "GUI", Control is HID "Control" — so a Mac
//! driving a PC sends Windows-C (which opens something entirely unrelated), and
//! a PC driving a Mac sends Control-C (which is not copy).
//!
//! Nothing here can be inferred: the protocol does not carry what the peer runs
//! on, and even if it did, plenty of people deliberately want the keys left
//! alone. So this is a choice the user makes per peer, and this module is only
//! the rule that choice names.
//!
//! The swap is applied by the controller before an event goes out, so the target
//! injects what it receives without needing to know anything.

use crate::input::{InputEvent, KeyCode, Modifiers};
use serde::{Deserialize, Serialize};

/// What to do with a peer's modifier keys before sending to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierSwap {
    /// Send the keys exactly as they were pressed.
    #[default]
    None,
    /// Exchange the Command/Windows key with Control, so muscle memory for
    /// shortcuts survives the trip between a Mac and a PC.
    MetaAndControl,
}

impl ModifierSwap {
    /// The swap named by a user-facing word, or `None` if it names nothing.
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "none" | "off" => Some(ModifierSwap::None),
            "meta-control" | "meta_control" | "cmd-ctrl" | "swap" | "on" => {
                Some(ModifierSwap::MetaAndControl)
            }
            _ => None,
        }
    }

    /// The word this swap is named by, for display and for the wire.
    pub const fn name(self) -> &'static str {
        match self {
            ModifierSwap::None => "none",
            ModifierSwap::MetaAndControl => "meta-control",
        }
    }

    /// Applies the swap to one event on its way to the peer.
    ///
    /// Both halves have to move together: the key *code*, so a press of Command
    /// arrives as a press of Control, and the `modifiers` set carried alongside
    /// every key, so a chord captured as Command-C arrives described as
    /// Control-C. Changing one without the other would leave the two disagreeing.
    pub fn apply(self, event: InputEvent) -> InputEvent {
        if self == ModifierSwap::None {
            return event;
        }
        match event {
            InputEvent::Key {
                code,
                action,
                modifiers,
            } => InputEvent::Key {
                code: swap_key(code),
                action,
                modifiers: swap_modifiers(modifiers),
            },
            // Mouse events carry no modifiers of their own; the target applies
            // whatever the keyboard events have already established.
            other => other,
        }
    }
}

/// Exchanges the GUI and Control keys, keeping the side they are on.
fn swap_key(code: KeyCode) -> KeyCode {
    match code.value() {
        0xE0 => KeyCode::new(0xE3), // Left Control  -> Left GUI
        0xE3 => KeyCode::new(0xE0), // Left GUI      -> Left Control
        0xE4 => KeyCode::new(0xE7), // Right Control -> Right GUI
        0xE7 => KeyCode::new(0xE4), // Right GUI     -> Right Control
        _ => code,
    }
}

/// Exchanges the Meta and Control bits, leaving Shift and Alt alone.
fn swap_modifiers(modifiers: Modifiers) -> Modifiers {
    let had_meta = modifiers.contains(Modifiers::META);
    let had_control = modifiers.contains(Modifiers::CONTROL);
    let mut swapped = modifiers
        .without(Modifiers::META)
        .without(Modifiers::CONTROL);
    if had_meta {
        swapped = swapped.with(Modifiers::CONTROL);
    }
    if had_control {
        swapped = swapped.with(Modifiers::META);
    }
    swapped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Action, MouseButton};

    fn key(code: u32, modifiers: Modifiers) -> InputEvent {
        InputEvent::Key {
            code: KeyCode::new(code),
            action: Action::Press,
            modifiers,
        }
    }

    #[test]
    fn no_swap_leaves_everything_alone() {
        let event = key(0xE3, Modifiers::META);
        assert_eq!(ModifierSwap::None.apply(event), event);
    }

    #[test]
    fn command_becomes_control_on_the_same_side() {
        let swap = ModifierSwap::MetaAndControl;

        // Left Command -> Left Control, right stays right.
        assert_eq!(
            swap.apply(key(0xE3, Modifiers::NONE)),
            key(0xE0, Modifiers::NONE)
        );
        assert_eq!(
            swap.apply(key(0xE7, Modifiers::NONE)),
            key(0xE4, Modifiers::NONE)
        );
        // And the other way, so the swap is its own inverse.
        assert_eq!(
            swap.apply(key(0xE0, Modifiers::NONE)),
            key(0xE3, Modifiers::NONE)
        );
    }

    #[test]
    fn a_command_chord_arrives_described_as_a_control_chord() {
        // Cmd-C on a Mac must reach a PC as Ctrl-C: the C key unchanged, but the
        // modifier set that describes it swapped.
        let swapped = ModifierSwap::MetaAndControl.apply(key(0x06, Modifiers::META));

        assert_eq!(swapped, key(0x06, Modifiers::CONTROL));
    }

    #[test]
    fn shift_and_alt_are_untouched() {
        let combo = Modifiers::SHIFT.with(Modifiers::ALT).with(Modifiers::META);

        let swapped = ModifierSwap::MetaAndControl.apply(key(0x06, combo));

        let expected = Modifiers::SHIFT
            .with(Modifiers::ALT)
            .with(Modifiers::CONTROL);
        assert_eq!(swapped, key(0x06, expected));
    }

    #[test]
    fn holding_both_leaves_both_held() {
        let both = Modifiers::META.with(Modifiers::CONTROL);

        let swapped = ModifierSwap::MetaAndControl.apply(key(0x06, both));

        assert_eq!(swapped, key(0x06, both));
    }

    #[test]
    fn applying_the_swap_twice_gets_the_original_back() {
        let swap = ModifierSwap::MetaAndControl;
        for code in [0xE0, 0xE3, 0xE4, 0xE7, 0x06] {
            for modifiers in [Modifiers::NONE, Modifiers::META, Modifiers::CONTROL] {
                let event = key(code, modifiers);
                assert_eq!(swap.apply(swap.apply(event)), event);
            }
        }
    }

    #[test]
    fn mouse_events_pass_through_untouched() {
        let click = InputEvent::Button {
            button: MouseButton::Left,
            action: Action::Press,
        };
        assert_eq!(ModifierSwap::MetaAndControl.apply(click), click);

        let pointer = InputEvent::Pointer { x: 3, y: 4 };
        assert_eq!(ModifierSwap::MetaAndControl.apply(pointer), pointer);
    }

    #[test]
    fn swaps_round_trip_through_their_names() {
        for swap in [ModifierSwap::None, ModifierSwap::MetaAndControl] {
            assert_eq!(ModifierSwap::parse(swap.name()), Some(swap));
        }
        // The friendly spellings a user is likely to type.
        assert_eq!(ModifierSwap::parse("off"), Some(ModifierSwap::None));
        assert_eq!(
            ModifierSwap::parse("cmd-ctrl"),
            Some(ModifierSwap::MetaAndControl)
        );
        assert_eq!(ModifierSwap::parse("sideways"), None);
    }

    #[test]
    fn the_default_is_to_change_nothing() {
        assert_eq!(ModifierSwap::default(), ModifierSwap::None);
    }
}
