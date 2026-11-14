//! The LocalDrop node: ties discovery, transport, protocol and storage together.
//!
//! This is the single entry point for front-ends. The Flutter app will call it through
//! `flutter_rust_bridge`, so the public API sticks to owned, FFI-friendly types and an
//! event channel instead of callbacks.

mod config;
mod events;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub use config::{NodeConfig, OutboxStorage};
pub use events::NodeEvent;
use localdrop_core::{DeviceId, DeviceIdentity, Error, GroupKey, Result, TransferId};
use localdrop_store::{FileSource, OutboxState, QueuedFile, QueuedTransfer, Store};
use tokio::sync::mpsc;

/// A running (or not yet started) LocalDrop node.
pub struct Node {
    config: NodeConfig,
    identity: DeviceIdentity,
    #[allow(dead_code)] // used by discovery/transport once implemented
    group_key: GroupKey,
    store: Arc<dyn Store>,
    events_tx: mpsc::UnboundedSender<NodeEvent>,
}

impl Node {
    /// Creates a node. Returns the node and the receiver for its events.
    pub fn new(
        config: NodeConfig,
        identity: DeviceIdentity,
        group_key: GroupKey,
        store: Arc<dyn Store>,
    ) -> (Self, mpsc::UnboundedReceiver<NodeEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (
            Self {
                config,
                identity,
                group_key,
                store,
                events_tx,
            },
            events_rx,
        )
    }

    /// This device's id.
    pub fn device_id(&self) -> DeviceId {
        self.identity.device_id()
    }

    /// Node configuration.
    pub fn config(&self) -> &NodeConfig {
        &self.config
    }

    /// Starts discovery and the listener; delivers queued transfers as peers appear.
    pub async fn start(&self) -> Result<()> {
        Err(Error::NotImplemented("node start"))
    }

    /// Stops all network activity.
    pub async fn stop(&self) -> Result<()> {
        Ok(())
    }

    /// Sends files to an online peer right now.
    pub async fn send(&self, _target: DeviceId, _files: Vec<PathBuf>) -> Result<TransferId> {
        Err(Error::NotImplemented("direct send"))
    }

    /// Queues files for a known device ("send ahead"). They are delivered automatically
    /// the next time the device is discovered and authenticated while this node runs.
    pub fn queue_send(&self, target: DeviceId, files: &[PathBuf]) -> Result<TransferId> {
        if self.store.device(&target)?.is_none() {
            return Err(Error::UnknownDevice(target));
        }
        let queued = files
            .iter()
            .map(|p| self.prepare_file(p))
            .collect::<Result<Vec<_>>>()?;
        let id = TransferId::random();
        self.store.enqueue(QueuedTransfer {
            id,
            target,
            files: queued,
            created_unix: now_unix(),
            expires_unix: None,
            state: OutboxState::Waiting,
        })?;
        tracing::info!(%id, %target, "transfer queued");
        let _ = self
            .events_tx
            .send(NodeEvent::TransferQueued { id, target });
        Ok(id)
    }

    /// Cancels a queued transfer.
    pub fn cancel_queued(&self, id: &TransferId) -> Result<()> {
        self.store.set_state(id, OutboxState::Cancelled)
    }

    /// Lists the outbox.
    pub fn queued(&self) -> Result<Vec<QueuedTransfer>> {
        self.store.transfers()
    }

    fn prepare_file(&self, path: &Path) -> Result<QueuedFile> {
        let path = std::fs::canonicalize(path)?;
        let meta = std::fs::metadata(&path)?;
        if !meta.is_file() {
            return Err(Error::Io(std::io::Error::other(
                "only regular files can be queued",
            )));
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or(Error::NotFound)?;

        let (source, hash_path) = match &self.config.outbox_storage {
            OutboxStorage::Reference => {
                let mtime_unix = meta
                    .modified()?
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                (
                    FileSource::Reference {
                        path: path.clone(),
                        size: meta.len(),
                        mtime_unix,
                    },
                    path,
                )
            }
            OutboxStorage::CopyToCache(dir) => {
                std::fs::create_dir_all(dir)?;
                let dest = dir.join(format!("{}-{}", TransferId::random(), name));
                std::fs::copy(&path, &dest)?;
                (
                    FileSource::Cached {
                        path: dest.clone(),
                        size: meta.len(),
                    },
                    dest,
                )
            }
        };

        let mut hasher = blake3::Hasher::new();
        hasher.update_reader(std::fs::File::open(&hash_path)?)?;
        Ok(QueuedFile {
            name,
            source,
            blake3: *hasher.finalize().as_bytes(),
        })
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use localdrop_core::{GroupConfig, KdfParams};
    use localdrop_store::{KnownDevice, KnownDevices, MemoryStore};

    use super::*;

    fn node(store: Arc<MemoryStore>) -> Node {
        let key = GroupKey::derive(
            &GroupConfig::new("home").unwrap(),
            "pw",
            KdfParams::INSECURE_TEST,
        )
        .unwrap();
        Node::new(
            NodeConfig::new("test"),
            DeviceIdentity::generate(),
            key,
            store,
        )
        .0
    }

    #[test]
    fn queue_requires_known_device() {
        let n = node(Arc::new(MemoryStore::new()));
        let err = n.queue_send(DeviceId([1; 16]), &[]).unwrap_err();
        assert!(matches!(err, Error::UnknownDevice(_)));
    }

    #[test]
    fn queue_known_device() {
        let store = Arc::new(MemoryStore::new());
        let pk = DeviceIdentity::generate().public_key();
        store
            .upsert_device(KnownDevice {
                id: pk.device_id(),
                public_key: pk,
                name: "phone".into(),
                last_seen_unix: 0,
                accept_override: None,
            })
            .unwrap();
        let n = node(store);

        let file = std::env::temp_dir().join(format!("localdrop-test-{}", TransferId::random()));
        std::fs::write(&file, b"hello").unwrap();
        let id = n
            .queue_send(pk.device_id(), std::slice::from_ref(&file))
            .unwrap();
        std::fs::remove_file(&file).unwrap();

        let q = n.queued().unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].id, id);
        assert_eq!(q[0].files[0].blake3, *blake3::hash(b"hello").as_bytes());
    }
}
