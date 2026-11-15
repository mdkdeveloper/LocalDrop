//! In-memory store.

use std::collections::BTreeMap;
use std::sync::Mutex;

use localdrop_core::{DeviceId, Error, JobId, Result};

use crate::{Direction, Job, JobState, Jobs, KnownDevice, KnownDevices};

/// Non-persistent [`KnownDevices`] + [`Jobs`] implementation.
#[derive(Debug, Default)]
pub struct MemoryStore {
    devices: Mutex<BTreeMap<DeviceId, KnownDevice>>,
    jobs: Mutex<JobTable>,
}

#[derive(Debug, Default)]
struct JobTable {
    list: Vec<Job>,
    next_position: u64,
}

impl MemoryStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    fn with_job<R>(&self, id: &JobId, f: impl FnOnce(&mut Job) -> R) -> Result<R> {
        let mut jobs = self.jobs.lock().expect("poisoned");
        let job = jobs
            .list
            .iter_mut()
            .find(|j| &j.id == id)
            .ok_or(Error::NotFound)?;
        Ok(f(job))
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

impl Jobs for MemoryStore {
    fn insert_job(&self, mut job: Job) -> Result<()> {
        let mut jobs = self.jobs.lock().expect("poisoned");
        if jobs.list.iter().any(|j| j.id == job.id) {
            return Err(Error::Protocol(format!("duplicate job id {}", job.id)));
        }
        // Positions start high so that `move_to_top` can always go below them.
        jobs.next_position += 1;
        job.position = u64::MAX / 2 + jobs.next_position;
        jobs.list.push(job);
        Ok(())
    }

    fn job(&self, id: &JobId) -> Result<Option<Job>> {
        let jobs = self.jobs.lock().expect("poisoned");
        Ok(jobs.list.iter().find(|j| &j.id == id).cloned())
    }

    fn jobs(&self) -> Result<Vec<Job>> {
        Ok(self.jobs.lock().expect("poisoned").list.clone())
    }

    fn queue(&self, peer: &DeviceId, direction: Direction) -> Result<Vec<Job>> {
        let jobs = self.jobs.lock().expect("poisoned");
        let mut v: Vec<_> = jobs
            .list
            .iter()
            .filter(|j| &j.peer == peer && j.direction == direction && j.state.is_active())
            .cloned()
            .collect();
        v.sort_by_key(|j| (j.position, j.created_unix));
        Ok(v)
    }

    fn set_state(&self, id: &JobId, state: JobState) -> Result<()> {
        self.with_job(id, |j| j.state = state)
    }

    fn set_progress(&self, id: &JobId, bytes_done: u64) -> Result<()> {
        self.with_job(id, |j| j.bytes_done = bytes_done)
    }

    fn move_to_top(&self, id: &JobId) -> Result<()> {
        let mut jobs = self.jobs.lock().expect("poisoned");
        let (peer, direction) = {
            let j = jobs
                .list
                .iter()
                .find(|j| &j.id == id)
                .ok_or(Error::NotFound)?;
            (j.peer, j.direction)
        };
        let min = jobs
            .list
            .iter()
            .filter(|j| j.peer == peer && j.direction == direction)
            .map(|j| j.position)
            .min()
            .unwrap_or(u64::MAX / 2);
        let job = jobs
            .list
            .iter_mut()
            .find(|j| &j.id == id)
            .expect("found above");
        job.position = min.saturating_sub(1);
        Ok(())
    }

    fn remove_job(&self, id: &JobId) -> Result<()> {
        self.jobs
            .lock()
            .expect("poisoned")
            .list
            .retain(|j| &j.id != id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use localdrop_core::{DeviceIdentity, PublicKey};
    use localdrop_protocol::Destination;

    use super::*;
    use crate::SourceStorage;

    fn device(pk: PublicKey) -> KnownDevice {
        KnownDevice {
            id: pk.device_id(),
            public_key: pk,
            name: "dev".into(),
            last_seen_unix: 0,
            accept_override: None,
        }
    }

    fn job(peer: DeviceId, direction: Direction, state: JobState) -> Job {
        Job {
            id: JobId::random(),
            peer,
            direction,
            name: "job".into(),
            destination: Destination::Inbox,
            deferred: false,
            entries: vec![],
            storage: SourceStorage::Reference,
            total_bytes: 0,
            bytes_done: 0,
            position: 0,
            created_unix: 0,
            expires_unix: None,
            state,
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
    fn queue_order_direction_and_state() {
        let store = MemoryStore::new();
        let peer = DeviceId([1; 16]);
        let first = job(peer, Direction::Outgoing, JobState::Waiting);
        let second = job(peer, Direction::Outgoing, JobState::Paused);
        let done = job(peer, Direction::Outgoing, JobState::Completed);
        let incoming = job(peer, Direction::Incoming, JobState::InProgress);
        let other_peer = job(DeviceId([2; 16]), Direction::Outgoing, JobState::Waiting);
        for j in [&first, &second, &done, &incoming, &other_peer] {
            store.insert_job((*j).clone()).unwrap();
        }
        let ids = |v: Vec<Job>| v.into_iter().map(|j| j.id).collect::<Vec<_>>();
        assert_eq!(
            ids(store.queue(&peer, Direction::Outgoing).unwrap()),
            [first.id, second.id]
        );
        assert_eq!(
            ids(store.queue(&peer, Direction::Incoming).unwrap()),
            [incoming.id]
        );

        store.move_to_top(&second.id).unwrap();
        assert_eq!(
            ids(store.queue(&peer, Direction::Outgoing).unwrap()),
            [second.id, first.id]
        );
        assert_eq!(store.jobs().unwrap().len(), 5);
    }

    #[test]
    fn duplicate_ids_rejected_and_updates_apply() {
        let store = MemoryStore::new();
        let j = job(DeviceId([1; 16]), Direction::Outgoing, JobState::Waiting);
        store.insert_job(j.clone()).unwrap();
        assert!(store.insert_job(j.clone()).is_err());

        store.set_state(&j.id, JobState::Cancelled).unwrap();
        store.set_progress(&j.id, 42).unwrap();
        let stored = store.job(&j.id).unwrap().unwrap();
        assert_eq!((stored.state, stored.bytes_done), (JobState::Cancelled, 42));

        store.remove_job(&j.id).unwrap();
        assert!(store.job(&j.id).unwrap().is_none());
        assert!(matches!(
            store.set_state(&j.id, JobState::Waiting),
            Err(Error::NotFound)
        ));
    }
}
