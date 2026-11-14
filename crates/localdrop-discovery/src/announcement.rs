//! Encrypted discovery announcement.
//!
//! Wire format (51 bytes, big-endian):
//!
//! ```text
//! +--------+-----+------+-----------+------------------------------+
//! | "LDRP" | ver | port |   nonce   |  ciphertext(device_id) + tag |
//! |   4    |  1  |  2   |    12     |           16 + 16            |
//! +--------+-----+------+-----------+------------------------------+
//! ```
//!
//! `ciphertext = ChaCha20-Poly1305(k_discovery, nonce, device_id, aad = magic||ver||port)`.
//!
//! - Outsiders cannot decrypt the device id and, because the nonce rotates, cannot link
//!   two announcements of the same device.
//! - Members detect "same group" by successful decryption (AEAD tag check).
//! - Binding the port in the AAD prevents tampering with it.
//! - Replays are harmless: discovery is only a hint, the handshake authenticates.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use localdrop_core::identity::DEVICE_ID_LEN;
use localdrop_core::{DeviceId, Error, GroupKey, Result};
use rand::RngCore;

/// Magic prefix identifying a LocalDrop announcement.
pub const MAGIC: &[u8; 4] = b"LDRP";
/// Current announcement format version.
pub const VERSION: u8 = 1;
/// Total encoded length.
pub const ENCODED_LEN: usize = 4 + 1 + 2 + 12 + DEVICE_ID_LEN + 16;

const HEADER_LEN: usize = 4 + 1 + 2;

/// An announcement as seen on the wire (opaque to non-members).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    bytes: [u8; ENCODED_LEN],
}

/// A successfully opened announcement from a member of our group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenedAnnouncement {
    /// Claimed device id of the announcer.
    pub device_id: DeviceId,
    /// Port the announcer listens on.
    pub port: u16,
}

impl Announcement {
    /// Seals an announcement for our device with a fresh random nonce.
    pub fn seal(key: &GroupKey, device_id: &DeviceId, port: u16) -> Self {
        let mut nonce = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        Self::seal_with_nonce(key, device_id, port, nonce)
    }

    fn seal_with_nonce(key: &GroupKey, device_id: &DeviceId, port: u16, nonce: [u8; 12]) -> Self {
        let mut bytes = [0u8; ENCODED_LEN];
        bytes[..4].copy_from_slice(MAGIC);
        bytes[4] = VERSION;
        bytes[5..7].copy_from_slice(&port.to_be_bytes());
        bytes[7..19].copy_from_slice(&nonce);

        let cipher = ChaCha20Poly1305::new(Key::from_slice(key.discovery_key()));
        let ct = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &device_id.0,
                    aad: &bytes[..HEADER_LEN],
                },
            )
            .expect("ChaCha20-Poly1305 encryption of 16 bytes cannot fail");
        bytes[19..].copy_from_slice(&ct);
        Self { bytes }
    }

    /// Parses raw bytes (does not check group membership).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ENCODED_LEN || &bytes[..4] != MAGIC || bytes[4] != VERSION {
            return Err(Error::InvalidAnnouncement);
        }
        let mut b = [0u8; ENCODED_LEN];
        b.copy_from_slice(bytes);
        Ok(Self { bytes: b })
    }

    /// Encoded bytes for sending (mDNS TXT uses base64url of these).
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Port in the clear (also present in the mDNS SRV record).
    pub fn port(&self) -> u16 {
        u16::from_be_bytes([self.bytes[5], self.bytes[6]])
    }

    /// Tries to open the announcement with our group key.
    ///
    /// Returns [`Error::InvalidAnnouncement`] for announcements from other groups.
    pub fn open(&self, key: &GroupKey) -> Result<OpenedAnnouncement> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(key.discovery_key()));
        let pt = cipher
            .decrypt(
                Nonce::from_slice(&self.bytes[7..19]),
                Payload {
                    msg: &self.bytes[19..],
                    aad: &self.bytes[..HEADER_LEN],
                },
            )
            .map_err(|_| Error::InvalidAnnouncement)?;
        let mut id = [0u8; DEVICE_ID_LEN];
        id.copy_from_slice(&pt);
        Ok(OpenedAnnouncement {
            device_id: DeviceId(id),
            port: self.port(),
        })
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

    #[test]
    fn same_group_opens() {
        let k = key("secret");
        let id = DeviceId([7; 16]);
        let ann = Announcement::seal(&k, &id, 4242);
        let parsed = Announcement::from_bytes(ann.as_bytes()).unwrap();
        assert_eq!(
            parsed.open(&k).unwrap(),
            OpenedAnnouncement {
                device_id: id,
                port: 4242
            }
        );
    }

    #[test]
    fn other_group_rejected() {
        let ann = Announcement::seal(&key("secret"), &DeviceId([7; 16]), 4242);
        assert!(ann.open(&key("other")).is_err());
    }

    #[test]
    fn tampered_port_rejected() {
        let k = key("secret");
        let ann = Announcement::seal(&k, &DeviceId([7; 16]), 4242);
        let mut bytes = ann.as_bytes().to_vec();
        bytes[6] ^= 1;
        assert!(Announcement::from_bytes(&bytes).unwrap().open(&k).is_err());
    }

    #[test]
    fn unlinkable_across_nonces() {
        let k = key("secret");
        let id = DeviceId([7; 16]);
        let a = Announcement::seal(&k, &id, 4242);
        let b = Announcement::seal(&k, &id, 4242);
        assert_ne!(a.as_bytes()[19..], b.as_bytes()[19..]);
    }

    #[test]
    fn garbage_rejected() {
        assert!(Announcement::from_bytes(b"hello").is_err());
        assert!(Announcement::from_bytes(&[0u8; ENCODED_LEN]).is_err());
    }
}
