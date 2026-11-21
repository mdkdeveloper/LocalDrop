//! An authenticated connection: handshake, `Hello`, and the control stream.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use localdrop_core::{DeviceId, Error, PublicKey, Result};
use localdrop_protocol::{
    Capabilities, ControlMessage, MAX_FRAME_LEN, PROTOCOL_VERSION, decode_frame, encode_frame,
};
use quinn::{ConnectionError, RecvStream, SendStream};
use tokio::sync::{mpsc, oneshot};

use crate::cert::certificate_key;
use crate::handshake::{self, Binding, EXPORTER_LABEL, Role};
use crate::quic::Local;

/// QUIC application close codes.
pub mod code {
    use quinn::VarInt;

    /// Normal close.
    pub const BYE: VarInt = VarInt::from_u32(0);
    /// Membership handshake failed.
    pub const AUTH_FAILED: VarInt = VarInt::from_u32(1);
    /// Different major protocol version.
    pub const INCOMPATIBLE: VarInt = VarInt::from_u32(2);
    /// The peer's key doesn't match the device it claims or was expected to be.
    pub const KEY_MISMATCH: VarInt = VarInt::from_u32(3);
    /// Another connection to the same device is kept instead.
    pub const DUPLICATE: VarInt = VarInt::from_u32(4);
    /// Protocol violation.
    pub const PROTOCOL: VarInt = VarInt::from_u32(5);
}

/// Time to wait for a `Pong`.
pub const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// The other side of a [`Session`], as authenticated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    /// Device id (derived from `public_key`).
    pub device_id: DeviceId,
    /// Public key from its TLS certificate.
    pub public_key: PublicKey,
    /// Name from its `Hello`.
    pub name: String,
    /// Capabilities from its `Hello`.
    pub capabilities: Capabilities,
    /// Remote address.
    pub addr: SocketAddr,
}

/// A connection that passed TLS, the membership handshake and `Hello`.
///
/// `Ping` and `Pong` are handled inside; every other control message is delivered on
/// [`Session::take_incoming`].
pub struct Session {
    conn: quinn::Connection,
    role: Role,
    local_id: DeviceId,
    peer: PeerInfo,
    writer: tokio::sync::Mutex<SendStream>,
    pings: Mutex<VecDeque<oneshot::Sender<()>>>,
    incoming: Mutex<Option<mpsc::UnboundedReceiver<ControlMessage>>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("role", &self.role)
            .field("peer", &self.peer)
            .finish_non_exhaustive()
    }
}

impl Session {
    /// The authenticated peer.
    pub fn peer(&self) -> &PeerInfo {
        &self.peer
    }

    /// Whether we opened the connection.
    pub fn role(&self) -> Role {
        self.role
    }

    /// Device id of the side that opened the connection.
    pub fn initiator(&self) -> DeviceId {
        match self.role {
            Role::Client => self.local_id,
            Role::Server => self.peer.device_id,
        }
    }

    /// Identifies the connection (for telling two sessions to one peer apart).
    pub fn stable_id(&self) -> usize {
        self.conn.stable_id()
    }

    /// Round-trip time as QUIC measures it.
    pub fn rtt(&self) -> Duration {
        self.conn.rtt()
    }

    /// Sends a control message.
    pub async fn send(&self, msg: &ControlMessage) -> Result<()> {
        let frame = encode_frame(msg)?;
        let mut w = self.writer.lock().await;
        w.write_all(&frame).await.map_err(|e| self.write_err(e))
    }

    /// Sends `Ping` and waits for the `Pong`.
    pub async fn ping(&self) -> Result<Duration> {
        let (tx, rx) = oneshot::channel();
        self.pings.lock().expect("poisoned").push_back(tx);
        let start = Instant::now();
        self.send(&ControlMessage::Ping).await?;
        match tokio::time::timeout(PING_TIMEOUT, rx).await {
            Ok(Ok(())) => Ok(start.elapsed()),
            Ok(Err(_)) => Err(self.closed_error()),
            Err(_) => Err(Error::Timeout),
        }
    }

    /// The control messages other than `Ping`/`Pong`; can be taken once.
    pub fn take_incoming(&self) -> Option<mpsc::UnboundedReceiver<ControlMessage>> {
        self.incoming.lock().expect("poisoned").take()
    }

    /// Whether the connection has ended.
    pub fn is_closed(&self) -> bool {
        self.conn.close_reason().is_some()
    }

    /// Waits until the connection ends and says why.
    pub async fn closed(&self) -> Error {
        let reason = self.conn.closed().await;
        close_error(&reason)
    }

    /// Closes a connection because another one to the same device is kept. The other
    /// side gets [`Error::AlreadyConnected`] with `local` (our id).
    pub fn close_duplicate(&self, local: DeviceId) {
        self.conn
            .close(code::DUPLICATE, local.to_string().as_bytes());
    }

    /// Closes the connection with one of the [`code`]s.
    pub fn close(&self, code: quinn::VarInt, reason: &str) {
        self.conn.close(code, reason.as_bytes());
    }

    fn closed_error(&self) -> Error {
        self.conn
            .close_reason()
            .map_or(Error::Transport("connection closed".into()), |r| {
                close_error(&r)
            })
    }

    fn write_err(&self, e: quinn::WriteError) -> Error {
        match e {
            quinn::WriteError::ConnectionLost(r) => close_error(&r),
            e => Error::Transport(e.to_string()),
        }
    }
}

/// Maps the reason a connection ended to an error.
pub(crate) fn close_error(e: &ConnectionError) -> Error {
    match e {
        ConnectionError::ApplicationClosed(c) => match c.error_code {
            code::AUTH_FAILED => Error::AuthFailed,
            code::INCOMPATIBLE => Error::IncompatibleVersion(
                std::str::from_utf8(&c.reason)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            ),
            code::KEY_MISMATCH => Error::Transport("the other device refused our key".into()),
            // The reason carries the closing side's device id (see `close_duplicate`).
            code::DUPLICATE => std::str::from_utf8(&c.reason)
                .ok()
                .and_then(|r| r.parse().ok())
                .map_or(
                    Error::Transport("replaced by another connection".into()),
                    Error::AlreadyConnected,
                ),
            _ => Error::Transport(String::from_utf8_lossy(&c.reason).into_owned()),
        },
        ConnectionError::TimedOut => Error::Timeout,
        e => Error::Transport(e.to_string()),
    }
}

pub(crate) fn conn_err(e: impl Into<ConnectError>) -> Error {
    match e.into() {
        ConnectError::Lost(c) => close_error(&c),
        ConnectError::Other(s) => Error::Transport(s),
    }
}

/// Errors while a connection is being set up.
pub(crate) enum ConnectError {
    Lost(ConnectionError),
    Other(String),
}

impl From<ConnectionError> for ConnectError {
    fn from(e: ConnectionError) -> Self {
        Self::Lost(e)
    }
}

impl From<quinn::ConnectError> for ConnectError {
    fn from(e: quinn::ConnectError) -> Self {
        Self::Other(e.to_string())
    }
}

/// After any failure, prefer the reason the peer gave for closing.
fn refine(conn: &quinn::Connection, e: Error) -> Error {
    match conn.close_reason() {
        Some(r @ ConnectionError::ApplicationClosed(_)) => close_error(&r),
        _ => e,
    }
}

async fn read_frame(recv: &mut RecvStream) -> Result<Option<ControlMessage>> {
    let mut len = [0u8; 4];
    match recv.read_exact(&mut len).await {
        Ok(()) => {}
        Err(quinn::ReadExactError::FinishedEarly(0)) => return Ok(None),
        Err(e) => return Err(Error::Transport(e.to_string())),
    }
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_FRAME_LEN {
        return Err(Error::Protocol("frame too large".into()));
    }
    let mut buf = vec![0u8; 4 + n];
    buf[..4].copy_from_slice(&len);
    recv.read_exact(&mut buf[4..])
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    Ok(decode_frame(&buf)?.map(|(m, _)| m))
}

async fn write_frame(send: &mut SendStream, msg: &ControlMessage) -> Result<()> {
    send.write_all(&encode_frame(msg)?)
        .await
        .map_err(|e| Error::Transport(e.to_string()))
}

/// Handshake, then `Hello` on the control stream.
pub(crate) async fn establish(
    conn: quinn::Connection,
    role: Role,
    local: &Local,
    expect: Option<DeviceId>,
) -> Result<Arc<Session>> {
    match establish_inner(&conn, role, local, expect).await {
        Ok(s) => Ok(s),
        Err(e) => {
            let e = refine(&conn, e);
            let (c, why) = match &e {
                Error::AuthFailed => (code::AUTH_FAILED, "not in this group".to_owned()),
                Error::KeyMismatch(_) => (code::KEY_MISMATCH, "key mismatch".to_owned()),
                Error::IncompatibleVersion(_) => (code::INCOMPATIBLE, PROTOCOL_VERSION.to_string()),
                _ => (code::PROTOCOL, "setup failed".to_owned()),
            };
            conn.close(c, why.as_bytes());
            Err(e)
        }
    }
}

async fn establish_inner(
    conn: &quinn::Connection,
    role: Role,
    local: &Local,
    expect: Option<DeviceId>,
) -> Result<Arc<Session>> {
    let theirs = conn
        .peer_identity()
        .and_then(|any| {
            any.downcast::<Vec<rustls::pki_types::CertificateDer<'static>>>()
                .ok()
        })
        .and_then(|certs| certs.first().and_then(|c| certificate_key(c)))
        .ok_or_else(|| Error::Transport("peer sent no device certificate".into()))?;
    let mut exporter = [0u8; 32];
    conn.export_keying_material(&mut exporter, EXPORTER_LABEL, b"")
        .map_err(|_| Error::Transport("TLS exporter unavailable".into()))?;

    // 1. Membership handshake on the first bidirectional stream.
    let (send, recv) = match role {
        Role::Client => conn.open_bi().await,
        Role::Server => conn.accept_bi().await,
    }
    .map_err(|e| close_error(&e))?;
    let mut stream = tokio::io::join(recv, send);
    let key = local.key.lock().expect("poisoned").clone();
    handshake::run(
        &mut stream,
        &key,
        role,
        Binding {
            exporter: &exporter,
            ours: local.public_key,
            theirs,
        },
    )
    .await?;
    let (_, mut send) = stream.into_inner();
    let _ = send.finish();

    // 2. Control stream: both sides send Hello first.
    let (mut send, mut recv) = match role {
        Role::Client => conn.open_bi().await,
        Role::Server => conn.accept_bi().await,
    }
    .map_err(|e| close_error(&e))?;
    let name = local.name.lock().expect("poisoned").clone();
    write_frame(
        &mut send,
        &ControlMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: local.device_id,
            device_name: name,
            capabilities: 0,
        },
    )
    .await?;
    let Some(ControlMessage::Hello {
        protocol_version,
        device_id,
        device_name,
        capabilities,
    }) = read_frame(&mut recv).await?
    else {
        return Err(Error::Protocol("expected Hello".into()));
    };
    if protocol_version != PROTOCOL_VERSION {
        return Err(Error::IncompatibleVersion(protocol_version));
    }
    let actual = theirs.device_id();
    if device_id != actual {
        return Err(Error::KeyMismatch(device_id));
    }
    if let Some(want) = expect.filter(|w| *w != actual) {
        return Err(Error::KeyMismatch(want));
    }

    let (tx, rx) = mpsc::unbounded_channel();
    let session = Arc::new(Session {
        conn: conn.clone(),
        role,
        local_id: local.device_id,
        peer: PeerInfo {
            device_id: actual,
            public_key: theirs,
            name: device_name,
            capabilities,
            addr: conn.remote_address(),
        },
        writer: tokio::sync::Mutex::new(send),
        pings: Mutex::new(VecDeque::new()),
        incoming: Mutex::new(Some(rx)),
    });
    tokio::spawn(reader(session.clone(), recv, tx));
    Ok(session)
}

/// Reads the control stream until it ends: answers pings, forwards the rest.
async fn reader(s: Arc<Session>, mut recv: RecvStream, tx: mpsc::UnboundedSender<ControlMessage>) {
    loop {
        match read_frame(&mut recv).await {
            Ok(Some(ControlMessage::Ping)) => {
                if s.send(&ControlMessage::Pong).await.is_err() {
                    break;
                }
            }
            Ok(Some(ControlMessage::Pong)) => {
                if let Some(p) = s.pings.lock().expect("poisoned").pop_front() {
                    let _ = p.send(());
                }
            }
            Ok(Some(ControlMessage::Hello { .. })) => {
                s.close(code::PROTOCOL, "second Hello");
                break;
            }
            Ok(Some(m)) => {
                let _ = tx.send(m);
            }
            Ok(None) => {
                s.close(code::BYE, "control stream ended");
                break;
            }
            Err(e) => {
                if !s.is_closed() {
                    tracing::debug!(peer = %s.peer.device_id, "control stream: {e}");
                    s.close(code::PROTOCOL, "bad control frame");
                }
                break;
            }
        }
    }
    s.pings.lock().expect("poisoned").clear();
}
