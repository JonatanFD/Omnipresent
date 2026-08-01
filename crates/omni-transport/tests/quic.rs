//! Live two-endpoint tests of the QUIC adapter over localhost: mutual TLS with
//! self-signed identities, policy enforcement during the handshake, datagrams,
//! and the control stream.

use omni_protocol::input::{Action, KeyCode, Modifiers};
use omni_protocol::{
    ClipboardData, ClipboardImage, ControlMessage, Fingerprint, InputEvent, Message, ScreenSize,
    SessionId,
};
use omni_security::{LocalIdentity, generate_identity};
use omni_transport::{
    HandshakePolicy, PolicyViolation, QuicConnection, QuicEndpoint, SecureChannel, Transport,
    TransportError,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// A policy that admits every peer — the "both sides already trust each other"
/// case, letting the tests focus on the channel itself.
struct AllowAll;

impl HandshakePolicy for AllowAll {
    fn authorize_server(&self, _host: &str, _fp: Fingerprint) -> Result<(), PolicyViolation> {
        Ok(())
    }

    fn authorize_client(&self, _fp: Fingerprint) -> Result<(), PolicyViolation> {
        Ok(())
    }
}

/// A policy that pins exactly one acceptable fingerprint in each direction.
struct PinnedOnly {
    server: Fingerprint,
    client: Fingerprint,
}

impl HandshakePolicy for PinnedOnly {
    fn authorize_server(&self, _host: &str, fp: Fingerprint) -> Result<(), PolicyViolation> {
        if fp == self.server {
            Ok(())
        } else {
            Err(PolicyViolation::new("fingerprint mismatch"))
        }
    }

    fn authorize_client(&self, fp: Fingerprint) -> Result<(), PolicyViolation> {
        if fp == self.client {
            Ok(())
        } else {
            Err(PolicyViolation::new("not allowed"))
        }
    }
}

fn localhost() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

fn endpoint(identity: &LocalIdentity, policy: impl HandshakePolicy + 'static) -> QuicEndpoint {
    QuicEndpoint::bind(localhost(), identity, Arc::new(policy)).expect("bind endpoint")
}

fn key_press(session: u128) -> Message {
    Message::Input {
        session: SessionId::new(session),
        event: InputEvent::Key {
            code: KeyCode::new(0x04),
            action: Action::Press,
            modifiers: Modifiers::SHIFT,
        },
    }
}

#[tokio::test]
async fn two_endpoints_exchange_datagrams_both_ways() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let alpha = endpoint(&alpha_id, AllowAll);
    let beta = endpoint(&beta_id, AllowAll);
    let beta_addr = beta.local_addr().unwrap();

    let (dialed, accepted) = tokio::join!(alpha.connect(beta_addr, "localhost"), async {
        beta.accept().await.expect("incoming connection")
    },);
    let dialed = dialed.expect("connect");
    let accepted = accepted.expect("accept");

    // Each side sees the other's certificate fingerprint.
    assert_eq!(dialed.peer_fingerprint(), beta_id.fingerprint());
    assert_eq!(accepted.peer_fingerprint(), alpha_id.fingerprint());

    // Frame Protocol messages over the connection in both directions.
    let mut controller = Transport::new(dialed);
    let mut target = Transport::new(accepted);

    controller.send(&key_press(1)).expect("send datagram");
    let received = recv_message(&mut target).await;
    assert_eq!(received, key_press(1));

    target.send(&key_press(2)).expect("send reply");
    let received = recv_message(&mut controller).await;
    assert_eq!(received, key_press(2));
}

/// Polls a transport until its datagram arrives (datagrams are unreliable but
/// loopback delivery is just asynchronous, not lossy).
async fn recv_message<C>(transport: &mut Transport<C>) -> Message
where
    C: SecureChannel,
    C::Error: std::fmt::Debug,
{
    for _ in 0..100 {
        match transport.recv() {
            Ok(Some(message)) => return message,
            Ok(None) => tokio::time::sleep(Duration::from_millis(10)).await,
            Err(TransportError::Channel(e)) => panic!("channel failed: {e:?}"),
            Err(TransportError::Codec(e)) => panic!("codec failed: {e}"),
        }
    }
    panic!("no datagram arrived within a second");
}

#[tokio::test]
async fn control_messages_ride_the_reliable_stream() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let alpha = endpoint(&alpha_id, AllowAll);
    let beta = endpoint(&beta_id, AllowAll);
    let beta_addr = beta.local_addr().unwrap();

    let (dialed, accepted) = tokio::join!(alpha.connect(beta_addr, "localhost"), async {
        beta.accept().await.expect("incoming connection")
    },);
    let dialed = dialed.unwrap();
    let accepted = accepted.unwrap();

    let request = Message::Control(ControlMessage::ConnectRequest {
        machine: omni_protocol::MachineId::new(7),
        fingerprint: alpha_id.fingerprint(),
        screen: ScreenSize::new(1920, 1080),
    });
    let accept = Message::Control(ControlMessage::Accept {
        session: SessionId::new(42),
        machine: omni_protocol::MachineId::new(9),
        screen: ScreenSize::new(2560, 1440),
    });

    let mut initiator_stream = dialed.open_control().await.expect("open control");
    initiator_stream.send(&request).await.expect("send request");

    let mut target_stream = accepted.accept_control().await.expect("accept control");
    assert_eq!(target_stream.recv().await.unwrap(), Some(request));

    target_stream.send(&accept).await.expect("send accept");
    assert_eq!(initiator_stream.recv().await.unwrap(), Some(accept));

    // Finishing the stream surfaces as a clean end on the other side.
    initiator_stream.finish();
    assert_eq!(target_stream.recv().await.unwrap(), None);
}

/// The two endpoints of a live connection, already handshaken.
async fn connected_pair(
    alpha_id: &LocalIdentity,
    beta_id: &LocalIdentity,
) -> (QuicConnection, QuicConnection) {
    let alpha = endpoint(alpha_id, AllowAll);
    let beta = endpoint(beta_id, AllowAll);
    let beta_addr = beta.local_addr().unwrap();

    let (dialed, accepted) = tokio::join!(alpha.connect(beta_addr, "localhost"), async {
        beta.accept().await.expect("incoming connection")
    },);
    (dialed.expect("connect"), accepted.expect("accept"))
}

/// A screenshot-sized clipboard payload: 1920x1080 in RGBA is about 8 MB, big
/// enough that writing it is not instant.
fn a_screenshot() -> Message {
    let width = 1920u32;
    let height = 1080u32;
    Message::Clipboard(ClipboardData::Image(ClipboardImage {
        width,
        height,
        bytes: vec![0xAB; (width * height * 4) as usize],
    }))
}

#[tokio::test]
async fn a_bulk_payload_arrives_on_its_own_stream() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let (dialed, mut accepted) = connected_pair(&alpha_id, &beta_id).await;

    let clipboard = a_screenshot();
    dialed
        .bulk_sender()
        .send(&clipboard)
        .await
        .expect("send bulk payload");

    let mut bulk = accepted
        .take_bulk_receiver()
        .expect("the bulk receiver is available once");
    assert_eq!(bulk.recv().await, Some(clipboard));
}

#[tokio::test]
async fn signalling_is_not_stuck_behind_a_bulk_transfer() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let (dialed, accepted) = connected_pair(&alpha_id, &beta_id).await;

    let mut initiator_stream = dialed.open_control().await.expect("open control");

    // A heartbeat leaves right after a big clipboard payload does. This is the
    // shape that used to drop sessions: sharing one stream put the heartbeat
    // behind the whole image, and the peer gave up waiting for it.
    let sender = dialed.bulk_sender();
    let image = tokio::spawn(async move { sender.send(&a_screenshot()).await });
    let heartbeat = Message::Control(ControlMessage::Heartbeat {
        session: SessionId::new(1),
    });
    initiator_stream
        .send(&heartbeat)
        .await
        .expect("send heartbeat");

    // The receiver reads only signalling — it never touches the bulk stream.
    // What comes off the control stream must be the heartbeat, not the image
    // that was sent first, and it must not have to wait for it either.
    let mut target_stream = accepted.accept_control().await.expect("accept control");
    let received = tokio::time::timeout(Duration::from_secs(5), target_stream.recv())
        .await
        .expect("heartbeat did not arrive while the image was being sent")
        .expect("read control stream");
    assert_eq!(received, Some(heartbeat));

    image.await.unwrap().expect("the image still went out");
}

#[tokio::test]
async fn the_control_stream_refuses_a_frame_too_big_for_signalling() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let (dialed, accepted) = connected_pair(&alpha_id, &beta_id).await;

    let mut initiator_stream = dialed.open_control().await.expect("open control");

    // Signalling is tiny. Anything image-sized belongs on a bulk stream, where
    // it cannot delay a heartbeat — so the control stream turns it away rather
    // than quietly growing to fit. Small enough here that the write itself
    // completes: what is under test is the refusal, not a stalled sender.
    let too_big = Message::Clipboard(ClipboardData::Image(ClipboardImage {
        width: 200,
        height: 100,
        bytes: vec![0xAB; 200 * 100 * 4],
    }));
    initiator_stream
        .send(&too_big)
        .await
        .expect("the sender writes it; the receiver is what refuses it");

    let mut target_stream = accepted.accept_control().await.expect("accept control");
    assert!(
        target_stream.recv().await.is_err(),
        "an image-sized frame was accepted as signalling"
    );
}

#[tokio::test]
async fn a_client_the_server_policy_refuses_cannot_connect() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let intruder_id = generate_identity("intruder").unwrap();

    // Beta only admits alpha's fingerprint, but the intruder dials.
    let beta = endpoint(
        &beta_id,
        PinnedOnly {
            server: alpha_id.fingerprint(),
            client: alpha_id.fingerprint(),
        },
    );
    let beta_addr = beta.local_addr().unwrap();
    let intruder = endpoint(&intruder_id, AllowAll);

    let (dialed, accepted) = tokio::join!(intruder.connect(beta_addr, "localhost"), async {
        // The refused handshake must never yield a connection on beta's side.
        tokio::time::timeout(Duration::from_millis(500), beta.accept())
            .await
            .ok()
    });

    if let Some(Some(result)) = accepted {
        assert!(result.is_err(), "refused client was accepted by the server");
    }

    // In TLS 1.3 the dialer can finish its side of the handshake before the
    // server has validated the client certificate, so the refusal may surface
    // either as a failed connect or as an immediate close.
    if let Ok(connection) = dialed {
        tokio::time::timeout(Duration::from_secs(5), connection.closed())
            .await
            .expect("refused client stayed connected");
    }
}

#[tokio::test]
async fn a_server_with_a_changed_fingerprint_is_refused_by_the_client() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let expected = generate_identity("the-beta-alpha-pinned-before").unwrap();

    // Alpha pinned a different certificate for this host: TOFU must refuse.
    let alpha = endpoint(
        &alpha_id,
        PinnedOnly {
            server: expected.fingerprint(),
            client: beta_id.fingerprint(),
        },
    );
    let beta = endpoint(&beta_id, AllowAll);
    let beta_addr = beta.local_addr().unwrap();

    let dialed = alpha.connect(beta_addr, "localhost").await;

    assert!(
        dialed.is_err(),
        "mismatched server fingerprint was accepted"
    );
}
