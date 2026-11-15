//! Node configuration.

use std::path::PathBuf;

use localdrop_core::AcceptSettings;

/// How files of jobs for offline devices are kept until delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxStorage {
    /// Keep a reference to the original path (desktop default).
    Reference,
    /// Copy into the given cache directory (mobile default: access to the original
    /// may be revoked once the share sheet / picker closes).
    CopyToCache(PathBuf),
}

impl Default for OutboxStorage {
    fn default() -> Self {
        if cfg!(any(target_os = "android", target_os = "ios")) {
            Self::CopyToCache(std::env::temp_dir().join("localdrop-outbox"))
        } else {
            Self::Reference
        }
    }
}

/// Node configuration (the subset of `AppSettings` the node needs today).
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Human-readable device name shown to peers.
    pub device_name: String,
    /// Where pushed files land (`inbox_dir`).
    pub inbox_dir: PathBuf,
    /// Where files from a sender's offline queue land; `None` = `inbox_dir`.
    pub deferred_inbox_dir: Option<PathBuf>,
    /// A job with several top-level items lands in its own subfolder of the inbox.
    pub job_subfolder: bool,
    /// QUIC listening port (0 = random, the default).
    pub listen_port: u16,
    /// Incoming accept policy.
    pub accept: AcceptSettings,
    /// Storage mode for jobs to offline devices.
    pub outbox_storage: OutboxStorage,
}

impl NodeConfig {
    /// Config with defaults for the current platform.
    pub fn new(device_name: impl Into<String>) -> Self {
        Self {
            device_name: device_name.into(),
            inbox_dir: std::env::temp_dir().join("localdrop-inbox"),
            deferred_inbox_dir: None,
            job_subfolder: true,
            listen_port: 0,
            accept: AcceptSettings::default(),
            outbox_storage: OutboxStorage::default(),
        }
    }
}
