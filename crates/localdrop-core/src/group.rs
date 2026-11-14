//! The shared group and the keys derived from its password.
//!
//! A LocalDrop device belongs to exactly one group. The group is defined by a
//! human-readable name and a shared password. Every device that knows both derives the
//! same [`GroupKey`] and is considered a member.
//!
//! Derivation (see `docs/security.md`):
//!
//! ```text
//! name_n   = normalize(name)                       // trim + Unicode lowercase
//! salt     = SHA-256("localdrop/v1/group-salt" || name_n)
//! master   = Argon2id(password, salt, params) -> 32 bytes
//! k_disc   = HKDF-SHA256(master, info = "localdrop/v1/discovery")
//! k_auth   = HKDF-SHA256(master, info = "localdrop/v1/auth")
//! k_fp     = HKDF-SHA256(master, info = "localdrop/v1/fingerprint")
//! ```
//!
//! The password itself is never stored; only the derived [`GroupKey`].

use std::fmt;

use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{Error, Result};

/// Maximum accepted group name length, in bytes after normalization.
pub const MAX_GROUP_NAME_LEN: usize = 64;

/// Public, non-secret group configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupConfig {
    /// Normalized group name.
    name: String,
}

impl GroupConfig {
    /// Creates a config, normalizing the name (trim + lowercase).
    pub fn new(name: &str) -> Result<Self> {
        let name = normalize_name(name)?;
        Ok(Self { name })
    }

    /// The normalized group name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

fn normalize_name(name: &str) -> Result<String> {
    let n = name.trim().to_lowercase();
    if n.is_empty() {
        return Err(Error::InvalidGroupName("empty"));
    }
    if n.len() > MAX_GROUP_NAME_LEN {
        return Err(Error::InvalidGroupName("too long"));
    }
    Ok(n)
}

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub m_cost_kib: u32,
    /// Number of iterations.
    pub t_cost: u32,
    /// Degree of parallelism.
    pub p_cost: u32,
}

impl KdfParams {
    /// Production parameters. Chosen to take well under a second on a mid-range phone
    /// while making offline guessing expensive. Changing them changes every group key,
    /// so they are part of protocol version 1.
    pub const V1: Self = Self {
        m_cost_kib: 64 * 1024,
        t_cost: 3,
        p_cost: 1,
    };

    /// Very cheap parameters for unit tests only. Never use in production.
    pub const INSECURE_TEST: Self = Self {
        m_cost_kib: 64,
        t_cost: 1,
        p_cost: 1,
    };
}

impl Default for KdfParams {
    fn default() -> Self {
        Self::V1
    }
}

/// Secret keys derived from the group password. Zeroized on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct GroupKey {
    discovery: [u8; 32],
    auth: [u8; 32],
    fingerprint: [u8; 32],
}

impl GroupKey {
    /// Derives the group key from a group name and password.
    pub fn derive(group: &GroupConfig, password: &str, params: KdfParams) -> Result<Self> {
        if password.is_empty() {
            return Err(Error::InvalidPassword("empty"));
        }

        let salt = Sha256::new()
            .chain_update(b"localdrop/v1/group-salt")
            .chain_update(group.name().as_bytes())
            .finalize();

        let argon_params = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(32))
            .map_err(|e| Error::Kdf(e.to_string()))?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);

        let mut master = [0u8; 32];
        argon
            .hash_password_into(password.as_bytes(), &salt, &mut master)
            .map_err(|e| Error::Kdf(e.to_string()))?;

        let hk = Hkdf::<Sha256>::new(None, &master);
        master.zeroize();

        let mut key = Self {
            discovery: [0; 32],
            auth: [0; 32],
            fingerprint: [0; 32],
        };
        hk.expand(b"localdrop/v1/discovery", &mut key.discovery)
            .map_err(|e| Error::Kdf(e.to_string()))?;
        hk.expand(b"localdrop/v1/auth", &mut key.auth)
            .map_err(|e| Error::Kdf(e.to_string()))?;
        hk.expand(b"localdrop/v1/fingerprint", &mut key.fingerprint)
            .map_err(|e| Error::Kdf(e.to_string()))?;
        Ok(key)
    }

    /// Key used to seal/open discovery announcements.
    pub fn discovery_key(&self) -> &[u8; 32] {
        &self.discovery
    }

    /// Key used as the PAKE password in the membership handshake.
    pub fn auth_key(&self) -> &[u8; 32] {
        &self.auth
    }

    /// Short human-comparable fingerprint (e.g. `3fa2-91c0`) so users can check that
    /// two devices typed the same name and password. Reveals nothing useful about the keys.
    pub fn fingerprint(&self) -> String {
        let f = &self.fingerprint;
        format!("{:02x}{:02x}-{:02x}{:02x}", f[0], f[1], f[2], f[3])
    }
}

impl fmt::Debug for GroupKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GroupKey({})", self.fingerprint())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str, pw: &str) -> GroupKey {
        GroupKey::derive(
            &GroupConfig::new(name).unwrap(),
            pw,
            KdfParams::INSECURE_TEST,
        )
        .unwrap()
    }

    #[test]
    fn deterministic() {
        let a = key("Home", "correct horse");
        let b = key("  home ", "correct horse");
        assert_eq!(a.discovery_key(), b.discovery_key());
        assert_eq!(a.auth_key(), b.auth_key());
        assert_eq!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn differs_by_password_and_name() {
        let a = key("home", "correct horse");
        assert_ne!(
            a.discovery_key(),
            key("home", "wrong horse").discovery_key()
        );
        assert_ne!(
            a.discovery_key(),
            key("work", "correct horse").discovery_key()
        );
    }

    #[test]
    fn subkeys_are_independent() {
        let a = key("home", "pw");
        assert_ne!(a.discovery_key(), a.auth_key());
    }

    #[test]
    fn rejects_empty() {
        assert!(GroupConfig::new("   ").is_err());
        let g = GroupConfig::new("home").unwrap();
        assert!(GroupKey::derive(&g, "", KdfParams::INSECURE_TEST).is_err());
    }
}
