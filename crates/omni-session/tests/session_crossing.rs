use omni_protocol::{MachineId, SessionId};
use omni_session::{
    ActiveTarget, RecordingEvents, Role, SessionError, SessionEvent, SessionManager,
};
use omni_topology::{Crossing, Point};

const LOCAL: MachineId = MachineId::new(1);
const PEER_A: MachineId = MachineId::new(2);
const PEER_B: MachineId = MachineId::new(3);
const S1: SessionId = SessionId::new(10);

fn manager() -> SessionManager<RecordingEvents> {
    SessionManager::new(LOCAL, RecordingEvents::default())
}

fn crossing(peer: MachineId) -> Crossing {
    Crossing {
        peer,
        entry: Point::new(0, 0),
    }
}

#[test]
fn crossing_onto_a_peer_routes_input_there() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();

    mgr.handle_crossing(crossing(PEER_A)).unwrap();

    assert_eq!(mgr.active_target(), ActiveTarget::Remote(PEER_A));
    assert_eq!(
        mgr.events().events().last(),
        Some(&SessionEvent::ActiveTargetChanged {
            target: ActiveTarget::Remote(PEER_A),
        }),
    );
}

#[test]
fn crossing_back_to_local_routes_input_home() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();
    mgr.handle_crossing(crossing(PEER_A)).unwrap();

    mgr.handle_crossing(crossing(LOCAL)).unwrap();

    assert_eq!(mgr.active_target(), ActiveTarget::Local);
}

#[test]
fn crossing_onto_an_unknown_peer_fails() {
    let mut mgr = manager();
    assert_eq!(
        mgr.handle_crossing(crossing(PEER_B)),
        Err(SessionError::NoSessionForPeer(PEER_B)),
    );
}

#[test]
fn repeated_crossing_to_the_same_target_emits_once() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();

    mgr.handle_crossing(crossing(PEER_A)).unwrap();
    mgr.handle_crossing(crossing(PEER_A)).unwrap();

    let changes = mgr
        .events()
        .events()
        .iter()
        .filter(|e| matches!(e, SessionEvent::ActiveTargetChanged { .. }))
        .count();
    assert_eq!(changes, 1);
}

#[test]
fn closing_the_active_session_returns_input_local() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();
    mgr.handle_crossing(crossing(PEER_A)).unwrap();

    mgr.close(S1).unwrap();

    assert!(mgr.is_empty());
    assert_eq!(mgr.active_target(), ActiveTarget::Local);
    let tail = mgr.events().events();
    assert_eq!(
        &tail[tail.len() - 2..],
        &[
            SessionEvent::ActiveTargetChanged {
                target: ActiveTarget::Local
            },
            SessionEvent::Closed { id: S1 },
        ],
    );
}

#[test]
fn a_peer_taking_over_brings_input_back_home() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();
    mgr.handle_crossing(crossing(PEER_A)).unwrap();

    assert!(mgr.yield_control());

    assert_eq!(mgr.active_target(), ActiveTarget::Local);
    assert_eq!(
        mgr.events().events().last(),
        Some(&SessionEvent::ActiveTargetChanged {
            target: ActiveTarget::Local,
        }),
    );
}

#[test]
fn following_the_peer_that_took_the_cursor_back_sends_input_there() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Target).unwrap();

    mgr.follow_peer(PEER_A).unwrap();

    assert_eq!(mgr.active_target(), ActiveTarget::Remote(PEER_A));
    assert_eq!(
        mgr.events().events().last(),
        Some(&SessionEvent::ActiveTargetChanged {
            target: ActiveTarget::Remote(PEER_A),
        }),
    );
}

#[test]
fn following_a_peer_we_have_no_session_with_fails_and_keeps_input_home() {
    let mut mgr = manager();

    assert_eq!(
        mgr.follow_peer(PEER_B),
        Err(SessionError::NoSessionForPeer(PEER_B)),
    );
    assert_eq!(mgr.active_target(), ActiveTarget::Local);
}

#[test]
fn control_travels_back_and_forth_with_only_one_end_driving() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();

    mgr.handle_crossing(crossing(PEER_A)).unwrap();
    assert_eq!(mgr.active_target(), ActiveTarget::Remote(PEER_A));

    mgr.yield_control();
    assert_eq!(mgr.active_target(), ActiveTarget::Local);

    mgr.follow_peer(PEER_A).unwrap();
    assert_eq!(mgr.active_target(), ActiveTarget::Remote(PEER_A));
}

#[test]
fn yielding_when_input_is_already_home_changes_nothing() {
    let mut mgr = manager();
    mgr.establish(S1, PEER_A, Role::Controller).unwrap();
    let before = mgr.events().events().len();

    assert!(!mgr.yield_control());

    assert_eq!(mgr.active_target(), ActiveTarget::Local);
    assert_eq!(mgr.events().events().len(), before);
}

#[test]
fn a_machine_with_no_sessions_can_still_be_asked_to_yield() {
    let mut mgr = manager();
    assert!(!mgr.yield_control());
    assert_eq!(mgr.active_target(), ActiveTarget::Local);
}
