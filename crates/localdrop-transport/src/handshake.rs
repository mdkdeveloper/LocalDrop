//! Membership handshake (docs/security.md#connection-authentication).
//!
//! Runs on the first QUIC bidirectional stream right after TLS completes:
//!
//! 1. Both sides compute `ctx = TLS-Exporter("EXPORTER-localdrop-v1", 32)` and
//!    `ids = sort(pk_client, pk_server)`.
//! 2. Symmetric SPAKE2 (Ed25519 group) with `k_auth` as the password and `ctx || ids` as
//!    the identity. The client sends its message first (33 bytes), then the server.
//! 3. Key confirmation: the client sends `HMAC-SHA256(K, "client" || ctx)`; the server
//!    checks it and only then answers with `HMAC-SHA256(K, "server" || ctx)`.
//!
//! An attacker without the password gets exactly one online guess per connection and
//! learns nothing useful for an offline dictionary attack. Binding to the TLS exporter
//! prevents relaying the handshake over another connection, and binding to both public
//! keys ties it to the certificates TLS verified.

use hmac::{Hmac, Mac};
use localdrop_core::{Error, GroupKey, PublicKey, Result};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Label used with the TLS keying-material exporter.
pub const EXPORTER_LABEL: &[u8] = b"EXPORTER-localdrop-v1";

/// Length of a SPAKE2 message (Ed25519 group, symmetric mode).
const SPAKE_MSG_LEN: usize = 33;
/// Length of a key confirmation tag.
const TAG_LEN: usize = 32;

/// Which side of the connection we are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Connection initiator.
    Client,
    /// Connection acceptor.
    Server,
}

impl Role {
    fn label(self) -> &'static [u8] {
        match self {
            Self::Client => b"client",
            Self::Server => b"server",
        }
    }

    fn other(self) -> Self {
        match self {
            Self::Client => Self::Server,
            Self::Server => Self::Client,
        }
    }
}

/// Inputs both sides must agree on.
pub struct Binding<'a> {
    /// TLS exporter output for [`EXPORTER_LABEL`].
    pub exporter: &'a [u8; 32],
    /// Our public key (from our certificate).
    pub ours: PublicKey,
    /// Their public key (from the certificate TLS verified).
    pub theirs: PublicKey,
}

fn identity(b: &Binding<'_>) -> Vec<u8> {
    let (lo, hi) = if b.ours.0 <= b.theirs.0 {
        (b.ours, b.theirs)
    } else {
        (b.theirs, b.ours)
    };
    let mut id = Vec::with_capacity(96);
    id.extend_from_slice(b.exporter);
    id.extend_from_slice(&lo.0);
    id.extend_from_slice(&hi.0);
    id
}

fn tag(key: &[u8], role: Role, exporter: &[u8; 32]) -> Hmac<Sha256> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("any key length");
    mac.update(role.label());
    mac.update(exporter);
    mac
}

fn io(e: std::io::Error) -> Error {
    // A closed stream mid-handshake means the other side refused us.
    match e.kind() {
        std::io::ErrorKind::UnexpectedEof
        | std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::ConnectionAborted
        | std::io::ErrorKind::BrokenPipe => Error::AuthFailed,
        _ => Error::Transport(e.to_string()),
    }
}

/// Runs the handshake over `stream`. `Ok` means both sides know the group key.
pub async fn run<S>(stream: &mut S, key: &GroupKey, role: Role, binding: Binding<'_>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (state, ours) = Spake2::<Ed25519Group>::start_symmetric(
        &Password::new(key.auth_key()),
        &Identity::new(&identity(&binding)),
    );
    debug_assert_eq!(ours.len(), SPAKE_MSG_LEN);
    let mut theirs = [0u8; SPAKE_MSG_LEN];
    match role {
        Role::Client => {
            stream.write_all(&ours).await.map_err(io)?;
            stream.flush().await.map_err(io)?;
            stream.read_exact(&mut theirs).await.map_err(io)?;
        }
        Role::Server => {
            stream.read_exact(&mut theirs).await.map_err(io)?;
            stream.write_all(&ours).await.map_err(io)?;
            stream.flush().await.map_err(io)?;
        }
    }
    let k = state.finish(&theirs).map_err(|_| Error::AuthFailed)?;

    let mine = tag(&k, role, binding.exporter).finalize().into_bytes();
    let check = |got: &[u8; TAG_LEN]| {
        tag(&k, role.other(), binding.exporter)
            .verify_slice(got)
            .map_err(|_| Error::AuthFailed)
    };
    let mut got = [0u8; TAG_LEN];
    match role {
        Role::Client => {
            stream.write_all(&mine).await.map_err(io)?;
            stream.flush().await.map_err(io)?;
            stream.read_exact(&mut got).await.map_err(io)?;
            check(&got)
        }
        Role::Server => {
            stream.read_exact(&mut got).await.map_err(io)?;
            check(&got)?;
            stream.write_all(&mine).await.map_err(io)?;
            stream.flush().await.map_err(io)
        }
    }
}

#[cfg(test)]
mod tests {
    use localdrop_core::{GroupConfig, KdfParams};

    use super::*;

    fn key(pw: &str) -> GroupKey {
        GroupKey::derive(
            &GroupConfig::new("home").unwrap(),
            pw,
            KdfParams::INSECURE_TEST,
        )
        .unwrap()
    }

    async fn both(
        kc: &GroupKey,
        ks: &GroupKey,
        ctx_c: [u8; 32],
        ctx_s: [u8; 32],
        swap_keys: bool,
    ) -> (Result<()>, Result<()>) {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let pc = PublicKey([1; 32]);
        let ps = PublicKey([2; 32]);
        let (theirs_for_server, ours_s) = if swap_keys {
            (PublicKey([3; 32]), ps)
        } else {
            (pc, ps)
        };
        let c = async {
            let r = run(
                &mut a,
                kc,
                Role::Client,
                Binding {
                    exporter: &ctx_c,
                    ours: pc,
                    theirs: ps,
                },
            )
            .await;
            drop(a);
            r
        };
        let s = async {
            let r = run(
                &mut b,
                ks,
                Role::Server,
                Binding {
                    exporter: &ctx_s,
                    ours: ours_s,
                    theirs: theirs_for_server,
                },
            )
            .await;
            drop(b);
            r
        };
        tokio::join!(c, s)
    }

    #[tokio::test]
    async fn same_password_succeeds() {
        let k = key("correct horse");
        let (c, s) = both(&k, &k, [7; 32], [7; 32], false).await;
        c.unwrap();
        s.unwrap();
    }

    #[tokio::test]
    async fn wrong_password_fails_on_both_sides() {
        let (c, s) = both(&key("a"), &key("b"), [7; 32], [7; 32], false).await;
        assert!(matches!(s, Err(Error::AuthFailed)));
        assert!(matches!(c, Err(Error::AuthFailed)));
    }

    #[tokio::test]
    async fn relayed_handshake_fails() {
        // Different TLS sessions give different exporters: a relay can't splice them.
        let k = key("pw");
        let (c, s) = both(&k, &k, [7; 32], [8; 32], false).await;
        assert!(c.is_err() && s.is_err());
    }

    #[tokio::test]
    async fn swapped_key_fails() {
        // The server sees another client key than the client used (key substitution).
        let k = key("pw");
        let (c, s) = both(&k, &k, [7; 32], [7; 32], true).await;
        assert!(c.is_err() && s.is_err());
    }
}
