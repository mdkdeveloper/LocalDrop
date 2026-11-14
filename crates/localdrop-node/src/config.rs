//! Node configuration.

use std::path::PathBuf;

use localdrop_core::AcceptSettings;

/// How queued (deferred) files are kept until delivery.
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

/// Node configuration.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Human-readable device name shown to peers.
    pub device_name: String,
    /// Directory where received files are written.
    pub download_dir: PathBuf,
    /// QUIC listening port (0 = any).
    pub listen_port: u16,
    /// Incoming accept policy.
    pub accept: AcceptSettings,
    /// Outbox storage mode.
    pub outbox_storage: OutboxStorage,
}

impl NodeConfig {
    /// Config with defaults for the current platform.
    pub fn new(device_name: impl Into<String>) -> Self {
        Self {
            device_name: device_name.into(),
            download_dir: std::env::temp_dir().join("localdrop-downloads"),
            listen_port: 0,
            accept: AcceptSettings::default(),
            outbox_storage: OutboxStorage::default(),
        }
    }
}
