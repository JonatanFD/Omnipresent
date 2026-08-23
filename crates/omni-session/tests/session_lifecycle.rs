use omni_protocol::{MachineId, SessionId};
use omni_session::{
    ActiveTarget, RecordingEvents, Role, SessionError, SessionEvent, SessionManager,
};

const LOCAL: MachineId = MachineId::new(1);
const PEER_A: MachineId = MachineId::new(2);
const PEER_B: MachineId = MachineId::new(3);
const S1: SessionId = SessionId::new(10);
const S2: SessionId = SessionId::new(20);

fn manager() -> SessionManager<RecordingEvents> {
    SessionManager::new(LOCAL, RecordingEvents::default())
}

#[test]
fn starts_local_with_no_sessions() {
    let mgr = manager();
    assert!(mgr.is_empty());
    assert_eq!(mgr.active_target(), ActiveTarget::Local);
}

#[test]
fn establishing_a_session_records_it_and_emits() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();

    assert_eq!(mgr.len(), 1);
    assert_eq!(mgr.session(S1).unwrap().peer, PEER_A);
    assert_eq!(mgr.session_for_peer(PEER_A).unwrap().id, S1);
    assert_eq!(
        mgr.events().events(),
        &[SessionEvent::Established {
            id: S1,
            peer: PEER_A,
            role: Role::Controller,
        }],
    );
}

#[test]
fn duplicate_session_id_is_rejected() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();
    assert_eq!(
        mgr.establish(S1, PEER_B, Role::Controller),
        Err(SessionError::DuplicateSession(S1)),
    );
}

#[test]
fn duplicate_peer_is_rejected() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();
    assert_eq!(
        mgr.establish(S2, PEER_A, Role::Controller),
        Err(SessionError::PeerAlreadyConnected(PEER_A)),
    );
}

#[test]
fn closing_an_unknown_session_fails() {
    let mut mgr = manager();
    assert_eq!(mgr.close(S1), Err(SessionError::UnknownSession(S1)));
}

#[test]
fn reversing_a_role_flips_and_emits() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();

    let role = mgr.reverse_role(S1).unwrap();

    assert_eq!(role, Role::Target);
    assert_eq!(mgr.session(S1).unwrap().role, Role::Target);
    assert_eq!(
        mgr.events().events().last(),
        Some(&SessionEvent::RoleChanged {
            id: S1,
            role: Role::Target,
        }),
    );
}

#[test]
fn reversing_an_unknown_session_fails() {
    let mut mgr = manager();
    assert_eq!(mgr.reverse_role(S1), Err(SessionError::UnknownSession(S1)));
}
