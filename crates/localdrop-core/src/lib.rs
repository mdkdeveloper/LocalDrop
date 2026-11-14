//! Core types shared by all LocalDrop crates.
//!
//! - [`identity`]: persistent per-install device identity (Ed25519) and [`DeviceId`].
//! - [`group`]: the shared group password and the keys derived from it.
//! - [`policy`]: incoming-transfer accept policy.
//! - [`ids`]: misc identifiers such as [`TransferId`].
//!
//! See `docs/architecture.md` and `docs/security.md` for the design.

pub mod error;
pub mod group;
pub mod identity;
pub mod ids;
pub mod policy;

pub use error::{Error, Result};
pub use group::{GroupConfig, GroupKey, KdfParams};
pub use identity::{DeviceId, DeviceIdentity, PublicKey};
pub use ids::TransferId;
pub use policy::{AcceptPolicy, AcceptSettings};
