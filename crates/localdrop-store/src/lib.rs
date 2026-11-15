//! Persistent state (see `docs/transfers.md` and `docs/deferred-send.md`).
//!
//! - [`KnownDevices`]: group members we have authenticated at least once, with their
//!   pinned public keys. Deferred jobs can only target these.
//! - [`Jobs`]: the pair queues in both directions, including deferred jobs waiting for
//!   an offline device, and finished jobs for history.
//!
//! [`MemoryStore`] is an in-memory implementation used by tests and the CLI skeleton.
//! A persistent `redb`-backed store follows in M3.

mod memory;
pub mod types;

use localdrop_core::{DeviceId, JobId, Result};
pub use memory::MemoryStore;
pub use types::{Direction, Job, JobEntry, JobState, KnownDevice, SourceStorage};

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

/// Jobs in all pair queues.
pub trait Jobs: Send + Sync {
    /// Adds a job at the end of its pair queue (its `position` is assigned here).
    fn insert_job(&self, job: Job) -> Result<()>;
    /// Looks up a job.
    fn job(&self, id: &JobId) -> Result<Option<Job>>;
    /// All jobs, active and finished.
    fn jobs(&self) -> Result<Vec<Job>>;
    /// Active jobs of one pair queue, in queue order.
    fn queue(&self, peer: &DeviceId, direction: Direction) -> Result<Vec<Job>>;
    /// Updates a job's state.
    fn set_state(&self, id: &JobId, state: JobState) -> Result<()>;
    /// Updates a job's progress.
    fn set_progress(&self, id: &JobId, bytes_done: u64) -> Result<()>;
    /// Moves a job to the front of its pair queue.
    fn move_to_top(&self, id: &JobId) -> Result<()>;
    /// Removes a job.
    fn remove_job(&self, id: &JobId) -> Result<()>;
}

/// Combined store used by the node.
pub trait Store: KnownDevices + Jobs {}
impl<T: KnownDevices + Jobs> Store for T {}
