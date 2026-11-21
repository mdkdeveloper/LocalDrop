//! QUIC endpoint: one UDP socket for incoming and outgoing connections.

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use localdrop_core::{DeviceId, DeviceIdentity, Error, GroupKey, PublicKey, Result};
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{IdleTimeout, TransportConfig, VarInt};

use crate::cert::{DeviceCertVerifier, SERVER_NAME, device_certificate, provider};
use crate::handshake::Role;
use crate::limit::RateLimiter;
use crate::session::{self, Session};

/// ALPN of protocol v1.
pub const ALPN: &[u8] = b"localdrop/1";

/// Time allowed for TLS, the membership handshake and `Hello` together.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Receive window per stream (docs/transfers.md#performance).
const STREAM_WINDOW: u32 = 16 * 1024 * 1024;
/// Receive and send window per connection.
const CONNECTION_WINDOW: u32 = 32 * 1024 * 1024;
/// Keep-alive ping; with the idle timeout this notices a vanished peer in ~20 s.
const KEEP_ALIVE: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(20);

type AllowFn = Arc<dyn Fn(IpAddr) -> bool + Send + Sync>;

/// What the endpoint knows about this device.
pub(crate) struct Local {
    pub key: Mutex<GroupKey>,
    pub public_key: PublicKey,
    pub device_id: DeviceId,
    pub name: Mutex<String>,
    limiter: Mutex<RateLimiter>,
    allow: RwLock<Option<AllowFn>>,
}

/// A QUIC endpoint that accepts and opens authenticated [`Session`]s.
#[derive(Clone)]
pub struct Endpoint {
    inner: quinn::Endpoint,
    local: Arc<Local>,
}

impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Endpoint")
            .field("device_id", &self.local.device_id)
            .field("addr", &self.inner.local_addr().ok())
            .finish()
    }
}

fn transport_config() -> TransportConfig {
    let mut t = TransportConfig::default();
    t.stream_receive_window(VarInt::from_u32(STREAM_WINDOW))
        .receive_window(VarInt::from_u32(CONNECTION_WINDOW))
        .send_window(u64::from(CONNECTION_WINDOW))
        .keep_alive_interval(Some(KEEP_ALIVE))
        .max_idle_timeout(Some(
            IdleTimeout::try_from(IDLE_TIMEOUT).expect("small timeout"),
        ));
    t
}

fn tls_err(e: impl std::fmt::Display) -> Error {
    Error::Transport(format!("TLS setup: {e}"))
}

impl Endpoint {
    /// Binds a UDP socket at `addr` (port 0 = any free port). Must run inside a tokio
    /// runtime. Until [`Endpoint::set_allow`] is called every source address is allowed.
    pub fn bind(
        addr: SocketAddr,
        identity: &DeviceIdentity,
        key: GroupKey,
        device_name: impl Into<String>,
    ) -> Result<Self> {
        let provider = provider();
        let verifier = DeviceCertVerifier::new(&provider);
        let (cert, private) = device_certificate(identity)?;
        let transport = Arc::new(transport_config());

        let mut server_tls = rustls::ServerConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(tls_err)?
            .with_client_cert_verifier(verifier.clone())
            .with_single_cert(vec![cert.clone()], private.clone_key())
            .map_err(tls_err)?;
        server_tls.alpn_protocols = vec![ALPN.to_vec()];
        let mut server = quinn::ServerConfig::with_crypto(Arc::new(
            QuicServerConfig::try_from(server_tls).map_err(tls_err)?,
        ));
        server.transport_config(transport.clone());

        let mut client_tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(tls_err)?
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_client_auth_cert(vec![cert], private)
            .map_err(tls_err)?;
        client_tls.alpn_protocols = vec![ALPN.to_vec()];
        let mut client = quinn::ClientConfig::new(Arc::new(
            QuicClientConfig::try_from(client_tls).map_err(tls_err)?,
        ));
        client.transport_config(transport);

        let mut inner = quinn::Endpoint::server(server, addr)?;
        inner.set_default_client_config(client);
        Ok(Self {
            inner,
            local: Arc::new(Local {
                key: Mutex::new(key),
                public_key: identity.public_key(),
                device_id: identity.device_id(),
                name: Mutex::new(device_name.into()),
                limiter: Mutex::new(RateLimiter::new()),
                allow: RwLock::new(None),
            }),
        })
    }

    /// The bound address (with the real port).
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.inner.local_addr()?)
    }

    /// This device's id.
    pub fn device_id(&self) -> DeviceId {
        self.local.device_id
    }

    /// Changes the group key used for new handshakes (after a password change).
    /// Existing sessions are not affected; close them if they must re-authenticate.
    pub fn set_key(&self, key: GroupKey) {
        *self.local.key.lock().expect("poisoned") = key;
    }

    /// Changes the name sent in `Hello` on new connections.
    pub fn set_device_name(&self, name: impl Into<String>) {
        *self.local.name.lock().expect("poisoned") = name.into();
    }

    /// Only connections from addresses for which `allow` returns true are accepted
    /// (the node passes "is on an active trusted subnet"). Others are refused before TLS.
    pub fn set_allow(&self, allow: impl Fn(IpAddr) -> bool + Send + Sync + 'static) {
        *self.local.allow.write().expect("poisoned") = Some(Arc::new(allow));
    }

    /// Connects to `addr` and authenticates. With `expect`, the peer must turn out to be
    /// that device, otherwise the result is [`Error::KeyMismatch`].
    pub async fn connect(
        &self,
        addr: SocketAddr,
        expect: Option<DeviceId>,
    ) -> Result<Arc<Session>> {
        let connecting = self
            .inner
            .connect(addr, SERVER_NAME)
            .map_err(|e| Error::Transport(e.to_string()))?;
        let local = self.local.clone();
        tokio::time::timeout(HANDSHAKE_TIMEOUT, async move {
            let conn = connecting.await.map_err(session::conn_err)?;
            session::establish(conn, Role::Client, &local, expect).await
        })
        .await
        .map_err(|_| Error::Timeout)?
    }

    /// Waits for the next incoming connection; `None` once the endpoint is closed.
    /// Connections from addresses that aren't allowed or are rate limited are refused
    /// here without a handshake.
    pub async fn accept(&self) -> Option<Incoming> {
        loop {
            let incoming = self.inner.accept().await?;
            let ip = incoming.remote_address().ip();
            let allowed = self
                .local
                .allow
                .read()
                .expect("poisoned")
                .as_ref()
                .is_none_or(|f| f(ip));
            let limited = !self
                .local
                .limiter
                .lock()
                .expect("poisoned")
                .allowed(ip, Instant::now());
            if !allowed || limited {
                tracing::debug!(%ip, allowed, limited, "refused connection");
                incoming.refuse();
                continue;
            }
            return Some(Incoming {
                incoming,
                local: self.local.clone(),
            });
        }
    }

    /// Closes every connection and stops accepting.
    pub fn close(&self) {
        self.inner.close(session::code::BYE, b"bye");
    }

    /// Waits until all connections are closed (after [`Endpoint::close`]).
    pub async fn wait_idle(&self) {
        self.inner.wait_idle().await;
    }
}

/// A connection attempt that passed the address checks.
pub struct Incoming {
    incoming: quinn::Incoming,
    local: Arc<Local>,
}

impl Incoming {
    /// Where it comes from.
    pub fn remote_address(&self) -> SocketAddr {
        self.incoming.remote_address()
    }

    /// Runs TLS, the membership handshake and `Hello`. Failed handshakes count towards
    /// the rate limit of the source address.
    pub async fn establish(self) -> Result<Arc<Session>> {
        let ip = self.incoming.remote_address().ip();
        let local = self.local.clone();
        let r = tokio::time::timeout(HANDSHAKE_TIMEOUT, async move {
            let conn = self
                .incoming
                .accept()
                .map_err(session::conn_err)?
                .await
                .map_err(session::conn_err)?;
            session::establish(conn, Role::Server, &local, None).await
        })
        .await
        .unwrap_or(Err(Error::Timeout));
        let mut limiter = self.local.limiter.lock().expect("poisoned");
        match &r {
            Ok(_) => limiter.succeeded(ip),
            Err(Error::AuthFailed) => limiter.failed(ip, Instant::now()),
            Err(_) => {}
        }
        r
    }
}
