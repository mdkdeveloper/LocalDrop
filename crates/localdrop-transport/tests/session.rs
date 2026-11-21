//! Real QUIC connections over loopback.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use localdrop_core::{DeviceIdentity, Error, GroupConfig, GroupKey, JobId, KdfParams};
use localdrop_protocol::ControlMessage;
use localdrop_transport::limit::MAX_FAILURES;
use localdrop_transport::{Endpoint, Session};

fn key(pw: &str) -> GroupKey {
    GroupKey::derive(
        &GroupConfig::new("home").unwrap(),
        pw,
        KdfParams::INSECURE_TEST,
    )
    .unwrap()
}

fn endpoint(pw: &str, name: &str) -> (Endpoint, DeviceIdentity) {
    let id = DeviceIdentity::generate();
    let ep = Endpoint::bind(
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        &id,
        key(pw),
        name,
    )
    .unwrap();
    (ep, id)
}

/// Accepts connections forever; sends each result to the channel.
fn serve(ep: &Endpoint) -> tokio::sync::mpsc::UnboundedReceiver<Result<Arc<Session>, Error>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let ep = ep.clone();
    tokio::spawn(async move {
        while let Some(inc) = ep.accept().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(inc.establish().await);
            });
        }
    });
    rx
}

#[tokio::test]
async fn members_connect_exchange_hello_and_ping() {
    let (server, sid) = endpoint("pw", "Desk PC");
    let (client, cid) = endpoint("pw", "Laptop");
    let mut accepted = serve(&server);

    let s = client
        .connect(server.local_addr().unwrap(), Some(sid.device_id()))
        .await
        .unwrap();
    assert_eq!(s.peer().device_id, sid.device_id());
    assert_eq!(s.peer().public_key, sid.public_key());
    assert_eq!(s.peer().name, "Desk PC");
    assert_eq!(s.initiator(), cid.device_id());

    let other = accepted.recv().await.unwrap().unwrap();
    assert_eq!(other.peer().device_id, cid.device_id());
    assert_eq!(other.peer().name, "Laptop");
    assert_eq!(other.initiator(), cid.device_id());

    s.ping().await.unwrap();
    other.ping().await.unwrap();

    // Other control messages arrive on the incoming channel.
    let mut inbox = other.take_incoming().unwrap();
    let msg = ControlMessage::JobPause {
        job_id: JobId([5; 16]),
    };
    s.send(&msg).await.unwrap();
    assert_eq!(inbox.recv().await.unwrap(), msg);

    s.close(localdrop_transport::code::BYE, "bye");
    let why = other.closed().await;
    assert!(matches!(why, Error::Transport(_)), "{why:?}");
}

#[tokio::test]
async fn wrong_password_is_refused_on_both_sides() {
    let (server, _) = endpoint("right", "Desk PC");
    let (client, _) = endpoint("wrong", "Laptop");
    let mut accepted = serve(&server);

    let err = client
        .connect(server.local_addr().unwrap(), None)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::AuthFailed), "{err:?}");
    let err = accepted.recv().await.unwrap().unwrap_err();
    assert!(matches!(err, Error::AuthFailed), "{err:?}");
}

#[tokio::test]
async fn unexpected_device_is_a_key_mismatch() {
    let (server, _) = endpoint("pw", "Desk PC");
    let (client, _) = endpoint("pw", "Laptop");
    let _accepted = serve(&server);
    let someone_else = DeviceIdentity::generate().device_id();

    let err = client
        .connect(server.local_addr().unwrap(), Some(someone_else))
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::KeyMismatch(d) if d == someone_else),
        "{err:?}"
    );
}

#[tokio::test]
async fn repeated_failures_are_rate_limited() {
    let (server, _) = endpoint("right", "Desk PC");
    let (client, _) = endpoint("wrong", "Laptop");
    let _accepted = serve(&server);
    let addr = server.local_addr().unwrap();

    for _ in 0..MAX_FAILURES {
        let err = client.connect(addr, None).await.unwrap_err();
        assert!(matches!(err, Error::AuthFailed), "{err:?}");
    }
    // Now refused before TLS, even with the right password.
    let (member, _) = endpoint("right", "Phone");
    let err = member.connect(addr, None).await.unwrap_err();
    assert!(!matches!(err, Error::AuthFailed), "{err:?}");
}

#[tokio::test]
async fn addresses_outside_the_allow_list_are_refused() {
    let (server, _) = endpoint("pw", "Desk PC");
    server.set_allow(|ip| !ip.is_loopback());
    let (client, _) = endpoint("pw", "Laptop");
    let _accepted = serve(&server);

    let err = client
        .connect(server.local_addr().unwrap(), None)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Transport(_)), "{err:?}");
}
