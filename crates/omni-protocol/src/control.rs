//! Control messages: the out-of-band signalling that sets up, maintains, and
//! tears down sessions. Distinct from the input stream, which carries the actual
//! keyboard and mouse events.

use crate::ids::{Fingerprint, MachineId, SessionId};
use serde::{Deserialize, Serialize};

/// The pixel dimensions of a machine's screen, exchanged during the handshake
/// so each side can place the other in its virtual desktop layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenSize {
    pub width: u32,
    pub height: u32,
}

impl ScreenSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// A signalling message between two machines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlMessage {
    /// "I would like to control you." Sent by the initiator; the target's user
    /// answers with `omni accept` / `omni reject`.
    ConnectRequest {
        machine: MachineId,
        fingerprint: Fingerprint,
        screen: ScreenSize,
    },
    /// The request was approved; the session now exists under this id. Carries
    /// the accepting machine's identity and screen so the initiator can place it
    /// in the layout.
    Accept {
        session: SessionId,
        machine: MachineId,
        screen: ScreenSize,
    },
    /// The request was refused, with the reason why.
    Reject { reason: RejectReason },
    /// Either side ends an established session.
    Disconnect { session: SessionId },
    /// Place the cursor at an absolute position on the receiver's screen —
    /// sent on edge crossings so the cursor appears exactly where it entered.
    /// Reliable (control stream), unlike the relative motion datagrams.
    ///
    /// It is also how control changes hands. The sender only ever sends this
    /// when its own cursor has just crossed onto the receiver, which is the same
    /// moment it starts sending input — so receiving one means "I am driving you
    /// now". A receiver that was itself driving a machine gives that up, leaving
    /// exactly one of the two in control. Without that, both ends could believe
    /// they were the controller at once and each would withhold input from its
    /// own desktop while sending it to the other, so neither would act on
    /// anything.
    CursorWarp { session: SessionId, x: i32, y: i32 },
    /// Keep-alive so each side can detect a silently dropped peer.
    Heartbeat { session: SessionId },
    /// "The cursor is back on my screen, at this position on it."
    ///
    /// The other half of the hand-over. [`ControlMessage::CursorWarp`] says
    /// *you* have the cursor now; this says *I* do, and it is sent at the moment
    /// the sender's cursor crosses back onto its own screen. Whoever holds the
    /// cursor is the machine every keyboard and mouse should be typing and
    /// moving on, so a receiver points its own input at the sender instead of at
    /// its local desktop.
    ///
    /// Without it only half the hand-over ever travelled: a machine learned when
    /// it had been taken over but never when the cursor had left again, so its
    /// keyboard stayed on its own desktop while the cursor sat on the other
    /// machine — typing landed on the wrong computer.
    ///
    /// The position is where the cursor landed on the *sender's* screen, so the
    /// receiver keeps tracking the one shared cursor from where it really is.
    ///
    /// New variants belong at the end: postcard encodes a variant as its
    /// position in this list, so inserting one in the middle would renumber the
    /// messages above it and a peer on an older version would misread every one
    /// of them.
    CursorReturned { session: SessionId, x: i32, y: i32 },
}

/// Why a [`ControlMessage::ConnectRequest`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    /// The peer is not on this machine's allowlist.
    NotAllowed,
    /// The peer's certificate fingerprint differs from the pinned one (TOFU).
    FingerprintChanged,
    /// A person explicitly declined the request.
    Declined,
    /// The machine is already in a session and cannot take another.
    Busy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_request_carries_who_is_asking() {
        let msg = ControlMessage::ConnectRequest {
            machine: MachineId::new(1),
            fingerprint: Fingerprint::from_bytes([9; 32]),
            screen: ScreenSize::new(1920, 1080),
        };

        match msg {
            ControlMessage::ConnectRequest {
                machine,
                fingerprint,
                screen,
            } => {
                assert_eq!(machine, MachineId::new(1));
                assert_eq!(fingerprint, Fingerprint::from_bytes([9; 32]));
                assert_eq!(screen, ScreenSize::new(1920, 1080));
            }
            _ => panic!("expected a connect request"),
        }
    }

    #[test]
    fn a_returned_cursor_says_where_it_landed_on_the_sender() {
        // The receiver needs the position to keep tracking the one cursor from
        // where it actually is, on the screen that now holds it.
        let msg = ControlMessage::CursorReturned {
            session: SessionId::new(7),
            x: 120,
            y: 640,
        };

        match msg {
            ControlMessage::CursorReturned { session, x, y } => {
                assert_eq!(session, SessionId::new(7));
                assert_eq!((x, y), (120, 640));
            }
            _ => panic!("expected a returned cursor"),
        }
    }

    #[test]
    fn accept_carries_the_target_identity_and_screen() {
        let msg = ControlMessage::Accept {
            session: SessionId::new(7),
            machine: MachineId::new(2),
            screen: ScreenSize::new(2560, 1440),
        };

        match msg {
            ControlMessage::Accept {
                session,
                machine,
                screen,
            } => {
                assert_eq!(session, SessionId::new(7));
                assert_eq!(machine, MachineId::new(2));
                assert_eq!(screen, ScreenSize::new(2560, 1440));
            }
            _ => panic!("expected an accept"),
        }
    }
}
