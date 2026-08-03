//! Live two-endpoint tests of the QUIC adapter over localhost: datagrams,
//! control streams, and bulk payloads.

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

struct AllowAll;

impl HandshakePolicy for AllowAll {
    fn authorize_server(&self, _host: &str, _fp: Fingerprint) -> Result<(), PolicyViolation> {
        Ok(())
    }

    fn authorize_client(&self, _fp: Fingerprint) -> Result<(), PolicyViolation> {
        Ok(())
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

fn a_screenshot() -> Message {
    let width = 1920u32;
    let height = 1080u32;
    Message::Clipboard(ClipboardData::Image(ClipboardImage {
        width,
        height,
        bytes: vec![0xAB; (width * height * 4) as usize],
    }))
}

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

    assert_eq!(dialed.peer_fingerprint(), beta_id.fingerprint());
    assert_eq!(accepted.peer_fingerprint(), alpha_id.fingerprint());

    let mut controller = Transport::new(dialed);
    let mut target = Transport::new(accepted);

    controller.send(&key_press(1)).expect("send datagram");
    let received = recv_message(&mut target).await;
    assert_eq!(received, key_press(1));

    target.send(&key_press(2)).expect("send reply");
    let received = recv_message(&mut controller).await;
    assert_eq!(received, key_press(2));
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

    initiator_stream.finish();
    assert_eq!(target_stream.recv().await.unwrap(), None);
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

    let sender = dialed.bulk_sender();
    let image = tokio::spawn(async move { sender.send(&a_screenshot()).await });
    let heartbeat = Message::Control(ControlMessage::Heartbeat {
        session: SessionId::new(1),
    });
    initiator_stream
        .send(&heartbeat)
        .await
        .expect("send heartbeat");

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
