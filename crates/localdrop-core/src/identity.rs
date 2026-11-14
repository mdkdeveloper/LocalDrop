//! Persistent device identity.
//!
//! Every LocalDrop install generates one Ed25519 keypair on first launch and keeps it
//! forever (until the user resets the app). The [`DeviceId`] is derived from the public
//! key, so a device cannot claim another device's id without its private key.

use std::fmt;
use std::str::FromStr;

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Length of a [`DeviceId`] in bytes.
pub const DEVICE_ID_LEN: usize = 16;

/// Ed25519 public key bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey(pub [u8; 32]);

impl PublicKey {
    /// The device id derived from this public key.
    pub fn device_id(&self) -> DeviceId {
        DeviceId::from_public_key(self)
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", self.device_id())
    }
}

/// Stable, public identifier of a device: the first 16 bytes of
/// `BLAKE3("localdrop/v1/device-id" || public_key)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceId(pub [u8; DEVICE_ID_LEN]);

impl DeviceId {
    /// Derives the id from a public key.
    pub fn from_public_key(pk: &PublicKey) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key("localdrop/v1/device-id");
        hasher.update(&pk.0);
        let hash = hasher.finalize();
        let mut id = [0u8; DEVICE_ID_LEN];
        id.copy_from_slice(&hash.as_bytes()[..DEVICE_ID_LEN]);
        Self(id)
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceId({self})")
    }
}

impl FromStr for DeviceId {
    type Err = Error;

    /// Parses the 32-character lowercase/uppercase hex form produced by `Display`.
    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.len() != DEVICE_ID_LEN * 2 || !s.is_ascii() {
            return Err(Error::InvalidKey);
        }
        let mut id = [0u8; DEVICE_ID_LEN];
        for (i, byte) in id.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| Error::InvalidKey)?;
        }
        Ok(Self(id))
    }
}

/// The local device's private identity.
pub struct DeviceIdentity {
    signing_key: SigningKey,
}

impl DeviceIdentity {
    /// Generates a fresh identity using the OS random number generator.
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::generate(&mut rand::rngs::OsRng),
        }
    }

    /// Restores an identity from its 32-byte secret seed.
    pub fn from_secret_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(bytes),
        }
    }

    /// Returns the 32-byte secret seed for persistent storage.
    ///
    /// Callers must store this in the platform's secure storage where available
    /// (Keychain, Android Keystore-wrapped file, DPAPI, Secret Service).
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// The public key.
    pub fn public_key(&self) -> PublicKey {
        PublicKey(self.signing_key.verifying_key().to_bytes())
    }

    /// The device id.
    pub fn device_id(&self) -> DeviceId {
        self.public_key().device_id()
    }

    /// Access to the underlying signing key (for TLS certificate generation, signatures).
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }
}

impl fmt::Debug for DeviceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceIdentity")
            .field("device_id", &self.device_id())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_roundtrip() {
        let id = DeviceIdentity::generate();
        let restored = DeviceIdentity::from_secret_bytes(&id.secret_bytes());
        assert_eq!(id.public_key(), restored.public_key());
        assert_eq!(id.device_id(), restored.device_id());
    }

    #[test]
    fn device_id_parse_display() {
        let id = DeviceIdentity::generate().device_id();
        let parsed: DeviceId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
        assert!("xyz".parse::<DeviceId>().is_err());
    }
}
