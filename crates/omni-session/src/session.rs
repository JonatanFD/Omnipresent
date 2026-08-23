//! Sessions, roles, and the manager that owns their lifecycle.

use crate::events::{SessionEvent, SessionEvents};
use omni_protocol::{MachineId, SessionId};
use omni_topology::Crossing;
use std::collections::HashMap;

/// This machine's part in a session. Reversible: a session that starts with this
/// machine controlling can be flipped so the peer controls instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// This machine's keyboard and mouse are the source of input.
    Controller,
    /// This machine receives and injects the peer's input.
    Target,
}

impl Role {
    /// The opposite role.
    pub const fn reversed(self) -> Role {
        match self {
            Role::Controller => Role::Target,
            Role::Target => Role::Controller,
        }
    }
}

/// One established session with a peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session {
    pub id: SessionId,
    pub peer: MachineId,
    pub role: Role,
}

/// Where input is currently going. When this machine is the Controller the
/// cursor moves across screens; `Local` means it is on this machine's own screen,
/// `Remote` means it has crossed onto a peer that is now receiving input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTarget {
    Local,
    Remote(MachineId),
}

/// Something the caller asked of the manager that does not make sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// A session with this id already exists.
    DuplicateSession(SessionId),
    /// A session with this peer already exists.
    PeerAlreadyConnected(MachineId),
    /// No session has this id.
    UnknownSession(SessionId),
    /// The cursor crossed onto a peer we have no session with.
    NoSessionForPeer(MachineId),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::DuplicateSession(id) => write!(f, "duplicate session {}", id.value()),
            SessionError::PeerAlreadyConnected(p) => {
                write!(f, "peer {} already connected", p.value())
            }
            SessionError::UnknownSession(id) => write!(f, "unknown session {}", id.value()),
            SessionError::NoSessionForPeer(p) => write!(f, "no session for peer {}", p.value()),
        }
    }
}

impl std::error::Error for SessionError {}

/// Owns the set of active sessions, the dynamic roles, and which target is
/// currently receiving input. Emits [`SessionEvent`]s through a
/// [`SessionEvents`] sink as things change.
#[derive(Debug)]
pub struct SessionManager<E: SessionEvents> {
    local: MachineId,
    sessions: HashMap<SessionId, Session>,
    by_peer: HashMap<MachineId, SessionId>,
    active: ActiveTarget,
    events: E,
}

impl<E: SessionEvents> SessionManager<E> {
    /// Creates a manager for this machine, with input starting on the local
    /// screen and no sessions.
    pub fn new(local: MachineId, events: E) -> Self {
        Self {
            local,
            sessions: HashMap::new(),
            by_peer: HashMap::new(),
            active: ActiveTarget::Local,
            events,
        }
    }

    /// The id of this machine.
    pub fn local(&self) -> MachineId {
        self.local
    }

    /// Where input is currently going.
    pub fn active_target(&self) -> ActiveTarget {
        self.active
    }

    /// The session with the given id, if any.
    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.get(&id)
    }

    /// The session with the given peer, if any.
    pub fn session_for_peer(&self, peer: MachineId) -> Option<&Session> {
        self.by_peer.get(&peer).and_then(|id| self.sessions.get(id))
    }

    /// How many sessions are active.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether there are no active sessions.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Read-only access to the events sink (mainly to inspect it in tests).
    pub fn events(&self) -> &E {
        &self.events
    }

    /// Establishes a new session after a connection is accepted. `role` is this
    /// machine's part: `Controller` if it initiated, `Target` if it accepted.
    pub fn establish(
        &mut self,
        id: SessionId,
        peer: MachineId,
        role: Role,
    ) -> Result<(), SessionError> {
        if self.sessions.contains_key(&id) {
            return Err(SessionError::DuplicateSession(id));
        }
        if self.by_peer.contains_key(&peer) {
            return Err(SessionError::PeerAlreadyConnected(peer));
        }
        self.sessions.insert(id, Session { id, peer, role });
        self.by_peer.insert(peer, id);
        self.events
            .emit(SessionEvent::Established { id, peer, role });
        Ok(())
    }

    /// Ends a session. If its peer was the active target, input returns to the
    /// local screen.
    pub fn close(&mut self, id: SessionId) -> Result<(), SessionError> {
        let session = self
            .sessions
            .remove(&id)
            .ok_or(SessionError::UnknownSession(id))?;
        self.by_peer.remove(&session.peer);
        if self.active == ActiveTarget::Remote(session.peer) {
            self.set_active(ActiveTarget::Local);
        }
        self.events.emit(SessionEvent::Closed { id });
        Ok(())
    }

    /// Reverses this machine's role in a session (Controller <-> Target), e.g.
    /// when the peer takes over control. Returns the new role.
    pub fn reverse_role(&mut self, id: SessionId) -> Result<Role, SessionError> {
        let session = self
            .sessions
            .get_mut(&id)
            .ok_or(SessionError::UnknownSession(id))?;
        session.role = session.role.reversed();
        let role = session.role;
        self.events.emit(SessionEvent::RoleChanged { id, role });
        Ok(role)
    }

    /// Brings input back to the local screen because a peer has taken control of
    /// this machine. Returns whether anything changed.
    ///
    /// Two machines may each hold a session with the other, and either one's
    /// user can push their cursor across at any moment. Without this, both ends
    /// could believe they were driving the other at the same time: each would
    /// send its input away and withhold it from its own desktop, so neither
    /// would act on what the other sent. The machine being taken over gives up
    /// what it was driving, which leaves exactly one of them in control.
    pub fn yield_control(&mut self) -> bool {
        let changed = self.active != ActiveTarget::Local;
        self.set_active(ActiveTarget::Local);
        changed
    }

    /// Sends this machine's input to `peer`, which has just taken the cursor
    /// back onto its own screen.
    ///
    /// The counterpart of [`yield_control`](Self::yield_control), and the other
    /// half of a hand-over. Giving way when a peer crosses onto this machine is
    /// only half the story: when the cursor leaves again, this machine has to
    /// hear about it too. Otherwise its keyboard goes on typing on its own
    /// desktop while the cursor — and the user's attention — sit on the peer.
    /// Whoever holds the cursor is where every keyboard and mouse should be
    /// working, whichever machine they happen to be plugged into.
    pub fn follow_peer(&mut self, peer: MachineId) -> Result<(), SessionError> {
        if !self.by_peer.contains_key(&peer) {
            return Err(SessionError::NoSessionForPeer(peer));
        }
        self.set_active(ActiveTarget::Remote(peer));
        Ok(())
    }

    /// Reacts to a cursor crossing reported by Topology, switching where input is
    /// routed. Crossing back onto this machine routes input locally; crossing
    /// onto a peer routes it there (the peer must have a session).
    pub fn handle_crossing(&mut self, crossing: Crossing) -> Result<(), SessionError> {
        if crossing.peer == self.local {
            self.set_active(ActiveTarget::Local);
            return Ok(());
        }
        if !self.by_peer.contains_key(&crossing.peer) {
            return Err(SessionError::NoSessionForPeer(crossing.peer));
        }
        self.set_active(ActiveTarget::Remote(crossing.peer));
        Ok(())
    }

    /// Updates the active target, emitting an event only when it actually changes.
    fn set_active(&mut self, target: ActiveTarget) {
        if self.active != target {
            self.active = target;
            self.events
                .emit(SessionEvent::ActiveTargetChanged { target });
        }
    }
}
