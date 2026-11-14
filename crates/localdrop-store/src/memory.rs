//! In-memory store.

use std::collections::BTreeMap;
use std::sync::Mutex;

use localdrop_core::{DeviceId, Error, Result, TransferId};

use crate::{KnownDevice, KnownDevices, Outbox, OutboxState, QueuedTransfer};

/// Non-persistent [`KnownDevices`] + [`Outbox`] implementation.
#[derive(Debug, Default)]
pub struct MemoryStore {
    devices: Mutex<BTreeMap<DeviceId, KnownDevice>>,
    outbox: Mutex<Vec<QueuedTransfer>>,
}

impl MemoryStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl KnownDevices for MemoryStore {
    fn upsert_device(&self, device: KnownDevice) -> Result<()> {
        let mut map = self.devices.lock().expect("poisoned");
        if let Some(existing) = map.get(&device.id) {
            if existing.public_key != device.public_key {
                return Err(Error::KeyMismatch(device.id));
            }
        }
        map.insert(device.id, device);
        Ok(())
    }

    fn device(&self, id: &DeviceId) -> Result<Option<KnownDevice>> {
        Ok(self.devices.lock().expect("poisoned").get(id).cloned())
    }

    fn devices(&self) -> Result<Vec<KnownDevice>> {
        Ok(self
            .devices
            .lock()
            .expect("poisoned")
            .values()
            .cloned()
            .collect())
    }

    fn remove_device(&self, id: &DeviceId) -> Result<()> {
        self.devices.lock().expect("poisoned").remove(id);
        Ok(())
    }
}

impl Outbox for MemoryStore {
    fn enqueue(&self, transfer: QueuedTransfer) -> Result<()> {
        self.outbox.lock().expect("poisoned").push(transfer);
        Ok(())
    }

    fn pending_for(&self, target: &DeviceId) -> Result<Vec<QueuedTransfer>> {
        let mut v: Vec<_> = self
            .outbox
            .lock()
            .expect("poisoned")
            .iter()
            .filter(|t| &t.target == target && t.state.is_active())
            .cloned()
            .collect();
        v.sort_by_key(|t| t.created_unix);
        Ok(v)
    }

    fn transfers(&self) -> Result<Vec<QueuedTransfer>> {
        Ok(self.outbox.lock().expect("poisoned").clone())
    }

    fn set_state(&self, id: &TransferId, state: OutboxState) -> Result<()> {
        let mut q = self.outbox.lock().expect("poisoned");
        let t = q.iter_mut().find(|t| &t.id == id).ok_or(Error::NotFound)?;
        t.state = state;
        Ok(())
    }

    fn remove_transfer(&self, id: &TransferId) -> Result<()> {
        self.outbox
            .lock()
            .expect("poisoned")
            .retain(|t| &t.id != id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use localdrop_core::{DeviceIdentity, PublicKey};

    use super::*;

    fn device(pk: PublicKey) -> KnownDevice {
        KnownDevice {
            id: pk.device_id(),
            public_key: pk,
            name: "dev".into(),
            last_seen_unix: 0,
            accept_override: None,
        }
    }

    #[test]
    fn key_pinning() {
        let store = MemoryStore::new();
        let pk = DeviceIdentity::generate().public_key();
        store.upsert_device(device(pk)).unwrap();
        store.upsert_device(device(pk)).unwrap();

        let mut forged = device(DeviceIdentity::generate().public_key());
        forged.id = pk.device_id();
        assert!(matches!(
            store.upsert_device(forged),
            Err(Error::KeyMismatch(_))
        ));
    }

    #[test]
    fn pending_filters_inactive() {
        let store = MemoryStore::new();
        let target = DeviceId([1; 16]);
        let mk = |created, state| QueuedTransfer {
            id: TransferId::random(),
            target,
            files: vec![],
            created_unix: created,
            expires_unix: None,
            state,
        };
        let newer = mk(2, OutboxState::Waiting);
        let older = mk(1, OutboxState::Waiting);
        let done = mk(0, OutboxState::Completed);
        for t in [newer.clone(), older.clone(), done] {
            store.enqueue(t).unwrap();
        }
        assert_eq!(store.pending_for(&target).unwrap(), vec![older, newer]);
    }
}
