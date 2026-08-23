//! Policy enforcement tests during the QUIC handshake: mutual TLS with
//! self-signed identities and fingerprint verification.

use omni_protocol::Fingerprint;
use omni_security::{LocalIdentity, generate_identity};
use omni_transport::{HandshakePolicy, PolicyViolation, QuicEndpoint};
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

#[tokio::test]
async fn a_client_the_server_policy_refuses_cannot_connect() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let intruder_id = generate_identity("intruder").unwrap();

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
        tokio::time::timeout(Duration::from_millis(500), beta.accept())
            .await
            .ok()
    });

    if let Some(Some(result)) = accepted {
        assert!(result.is_err(), "refused client was accepted by the server");
    }

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
