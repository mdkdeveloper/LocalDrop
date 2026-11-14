//! Persistent state (see `docs/deferred-send.md`).
//!
//! - [`KnownDevices`]: group members we have authenticated at least once, with their
//!   pinned public keys. Deferred sends can only target these.
//! - [`Outbox`]: transfers queued for devices that are currently offline.
//!
//! [`MemoryStore`] is an in-memory implementation used by tests and the CLI skeleton.
//! A persistent `redb`-backed store is planned.

mod memory;
pub mod types;

use localdrop_core::{DeviceId, Result, TransferId};
pub use memory::MemoryStore;
pub use types::{FileSource, KnownDevice, OutboxState, QueuedFile, QueuedTransfer};

/// Registry of known group members.
pub trait KnownDevices: Send + Sync {
    /// Inserts or updates a device.
    ///
    /// Returns [`Error::KeyMismatch`](localdrop_core::Error::KeyMismatch) if a device with
    /// the same id is already stored with a different public key.
    fn upsert_device(&self, device: KnownDevice) -> Result<()>;
    /// Looks up a device.
    fn device(&self, id: &DeviceId) -> Result<Option<KnownDevice>>;
    /// All known devices.
    fn devices(&self) -> Result<Vec<KnownDevice>>;
    /// Forgets a device (and its pinned key).
    fn remove_device(&self, id: &DeviceId) -> Result<()>;
}

/// Queue of deferred transfers on the sender side.
pub trait Outbox: Send + Sync {
    /// Adds a transfer to the queue.
    fn enqueue(&self, transfer: QueuedTransfer) -> Result<()>;
    /// Unfinished transfers for a given target, oldest first.
    fn pending_for(&self, target: &DeviceId) -> Result<Vec<QueuedTransfer>>;
    /// All transfers in the outbox.
    fn transfers(&self) -> Result<Vec<QueuedTransfer>>;
    /// Updates a transfer's state.
    fn set_state(&self, id: &TransferId, state: OutboxState) -> Result<()>;
    /// Removes a transfer.
    fn remove_transfer(&self, id: &TransferId) -> Result<()>;
}

/// Combined store used by the node.
pub trait Store: KnownDevices + Outbox {}
impl<T: KnownDevices + Outbox> Store for T {}
