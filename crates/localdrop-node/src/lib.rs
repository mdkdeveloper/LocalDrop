//! The LocalDrop node: ties discovery, transport, protocol and storage together.
//!
//! This is the single entry point for front-ends. The Flutter app calls it through
//! `flutter_rust_bridge`, so the public API sticks to owned, FFI-friendly types
//! ([`api`]) and one event channel ([`NodeEvent`]) instead of callbacks. The full
//! surface, including what later milestones fill in, is described in `docs/node-api.md`.
//!
//! With the `fake` feature, [`fake`] provides scripted events for UI work without a
//! network.

pub mod api;
mod config;
mod events;
#[cfg(feature = "fake")]
pub mod fake;
mod manifest;
mod network;
pub mod settings;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

pub use api::{
    AcceptAnswer, ConflictMode, Destination, DeviceView, GroupInfo, JobAction, JobView,
    NetworkTrust, NetworkView, Presence, SendRequest, SourceChangedChoice,
};
pub use config::{NodeConfig, OutboxStorage};
pub use events::NodeEvent;
use localdrop_core::{
    AcceptPolicy, DeviceId, DeviceIdentity, Error, GroupConfig, GroupKey, JobId, Result, ShareId,
};
use localdrop_protocol::{Cursor, DirEntry, SearchHit, ShareInfo};
pub use localdrop_store::{Direction, JobState};
use localdrop_store::{Job, SourceStorage, Store};
pub use settings::{NetworkSelection, OnNewNetwork, Settings, StateDir};
use tokio::sync::mpsc;

use crate::network::{Shared, key_matches};

/// A running (or not yet started) LocalDrop node.
pub struct Node {
    config: NodeConfig,
    identity: DeviceIdentity,
    group: GroupConfig,
    group_key: GroupKey,
    store: Arc<dyn Store>,
    events_tx: mpsc::UnboundedSender<NodeEvent>,
    shared: Arc<Shared>,
}

impl Node {
    /// Creates a node for a joined group with settings kept in memory only. Returns the
    /// node and the receiver for its events.
    pub fn new(
        config: NodeConfig,
        identity: DeviceIdentity,
        group: GroupConfig,
        group_key: GroupKey,
        store: Arc<dyn Store>,
    ) -> (Self, mpsc::UnboundedReceiver<NodeEvent>) {
        let settings = Settings::new(config.device_name.clone(), group.name());
        Self::build(config, identity, group, group_key, store, settings, None)
    }

    /// Opens the node of a joined [`StateDir`]; settings changes are saved there.
    pub fn open(
        dir: StateDir,
        store: Arc<dyn Store>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<NodeEvent>)> {
        let joined = dir.load()?;
        let mut config = NodeConfig::new(joined.settings.device_name.clone());
        config.listen_port = joined.settings.listen_port;
        Ok(Self::build(
            config,
            joined.identity,
            joined.group,
            joined.key,
            store,
            joined.settings,
            Some(dir),
        ))
    }

    fn build(
        config: NodeConfig,
        identity: DeviceIdentity,
        group: GroupConfig,
        group_key: GroupKey,
        store: Arc<dyn Store>,
        settings: Settings,
        dir: Option<StateDir>,
    ) -> (Self, mpsc::UnboundedReceiver<NodeEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared::new(
            settings,
            dir,
            events_tx.clone(),
            identity.device_id(),
            group_key.clone(),
        ));
        (
            Self {
                config,
                identity,
                group,
                group_key,
                store,
                events_tx,
                shared,
            },
            events_rx,
        )
    }

    // -- Lifecycle ----------------------------------------------------------------------

    /// This device's id.
    pub fn device_id(&self) -> DeviceId {
        self.identity.device_id()
    }

    /// Node configuration.
    pub fn config(&self) -> &NodeConfig {
        &self.config
    }

    /// A copy of the current settings.
    pub fn settings(&self) -> Settings {
        self.shared.settings.lock().expect("poisoned").clone()
    }

    /// Starts discovery on the trusted networks and watches for network changes.
    /// Must be called inside a tokio runtime. Connections and queue delivery follow in M2.
    pub async fn start(&self) -> Result<()> {
        self.shared.start().await;
        Ok(())
    }

    /// Stops all network activity. Queues are kept.
    pub async fn stop(&self) -> Result<()> {
        self.shared.stop().await;
        Ok(())
    }

    /// The discovery switch on the main screen. Lasts until the next network change or
    /// restart; then *Discovery at launch* applies again.
    pub async fn set_discovery(&self, on: bool) -> Result<()> {
        self.shared.discovery_on.store(on, Ordering::SeqCst);
        self.shared.reconcile().await;
        Ok(())
    }

    /// Whether the discovery switch is on.
    pub fn discovery_on(&self) -> bool {
        self.shared.discovery_on.load(Ordering::SeqCst)
    }

    /// *Visible to your group*: announce, or only browse.
    pub async fn set_visible(&self, visible: bool) -> Result<()> {
        self.update_settings(|s| s.visible = visible)?;
        self.shared.reconcile().await;
        Ok(())
    }

    // -- Group --------------------------------------------------------------------------

    /// Group name and fingerprint.
    pub fn group_info(&self) -> GroupInfo {
        GroupInfo {
            name: self.group.name().to_owned(),
            fingerprint: self.group_key.fingerprint(),
        }
    }

    /// Changes the group password after checking the current one. (M2)
    pub async fn change_password(&self, _current: String, _new: String) -> Result<GroupInfo> {
        Err(Error::NotImplemented("change password"))
    }

    /// Leaves the group. (M2)
    pub async fn leave_group(&self, _keep_queue: bool, _forget_devices: bool) -> Result<()> {
        Err(Error::NotImplemented("leave group"))
    }

    // -- Devices ------------------------------------------------------------------------

    /// Known devices, plus group members discovered but not yet authenticated (named by
    /// their short id until the handshake tells their name, M2).
    pub fn devices(&self) -> Result<Vec<DeviceView>> {
        let found = self.shared.peers.lock().expect("poisoned").clone();
        let mut out: Vec<DeviceView> = self
            .store
            .devices()?
            .into_iter()
            .map(|d| {
                Ok(DeviceView {
                    queued_jobs: self.store.queue(&d.id, Direction::Outgoing)?.len() as u32,
                    presence: if found.contains_key(&d.id) {
                        Presence::Found
                    } else {
                        Presence::Offline
                    },
                    id: d.id,
                    name: d.name,
                    last_seen_unix: d.last_seen_unix,
                    accept_override: d.accept_override,
                })
            })
            .collect::<Result<_>>()?;
        for id in found.keys() {
            if !out.iter().any(|d| &d.id == id) {
                out.push(DeviceView {
                    id: *id,
                    name: format!("Device {}", &id.to_string()[..8]),
                    presence: Presence::Found,
                    last_seen_unix: 0,
                    accept_override: None,
                    queued_jobs: 0,
                });
            }
        }
        Ok(out)
    }

    /// Addresses a discovered peer announced (empty if not on the network).
    pub fn peer_addrs(&self, device: DeviceId) -> Vec<std::net::SocketAddr> {
        self.shared
            .peers
            .lock()
            .expect("poisoned")
            .get(&device)
            .cloned()
            .unwrap_or_default()
    }

    /// Sets or clears a device's accept override (`None` = like the app default).
    pub fn set_device_accept(&self, device: DeviceId, policy: Option<AcceptPolicy>) -> Result<()> {
        let mut d = self
            .store
            .device(&device)?
            .ok_or(Error::UnknownDevice(device))?;
        d.accept_override = policy;
        self.store.upsert_device(d)
    }

    /// Forgets a device and its pinned key. Its queued jobs are cancelled.
    pub fn forget_device(&self, device: DeviceId) -> Result<()> {
        for job in self.store.queue(&device, Direction::Outgoing)? {
            self.job_action(job.id, JobAction::Cancel)?;
        }
        self.store.remove_device(&device)
    }

    /// *Connect by address*: `ip[:port]` or a host name. Needs the handshake. (M2)
    pub async fn connect_address(&self, _address: String, _remember: bool) -> Result<()> {
        Err(Error::NotImplemented("connect by address"))
    }

    // -- Networks -----------------------------------------------------------------------

    /// Connected networks (trusted or not) and remembered ones.
    pub fn networks(&self) -> Result<Vec<NetworkView>> {
        Ok(self.shared.current_plan().0.views)
    }

    /// Trusts or distrusts a network, by its key (full hex, or the first 8 characters).
    /// The network must be connected or remembered.
    pub async fn set_network_trust(&self, key: String, trust: NetworkTrust) -> Result<()> {
        let (_, conns) = self.shared.current_plan();
        let connected = conns.into_iter().find(|c| key_matches(&c.id.key, &key));
        self.update_settings(|s| {
            if let Some(c) = &connected {
                let label = s
                    .network(&c.id.key)
                    .map_or_else(|| c.id.default_label.clone(), |n| n.label.clone());
                s.set_trust(c.id.key, &label, c.id.kind, c.id.weak, trust);
                Ok(())
            } else if let Some(n) = s
                .known_networks
                .iter_mut()
                .find(|n| key_matches(&n.key, &key))
            {
                n.trust = trust;
                Ok(())
            } else {
                Err(Error::NotFound)
            }
        })??;
        self.shared.reconcile().await;
        Ok(())
    }

    /// Renames a remembered network.
    pub fn rename_network(&self, key: String, label: String) -> Result<()> {
        self.update_settings(|s| {
            s.known_networks
                .iter_mut()
                .find(|n| key_matches(&n.key, &key))
                .map(|n| n.label = label)
                .ok_or(Error::NotFound)
        })?
    }

    /// Forgets a remembered network (it is asked about again next time).
    pub fn forget_network(&self, key: String) -> Result<()> {
        self.update_settings(|s| s.known_networks.retain(|n| !key_matches(&n.key, &key)))
    }

    /// Changes the network settings (trust every network, selection, …).
    pub async fn set_network_settings(
        &self,
        f: impl FnOnce(&mut settings::NetworkSettings),
    ) -> Result<()> {
        self.update_settings(|s| f(&mut s.networks))?;
        self.shared.reconcile().await;
        Ok(())
    }

    fn update_settings<R>(&self, f: impl FnOnce(&mut Settings) -> R) -> Result<R> {
        let r = f(&mut self.shared.settings.lock().expect("poisoned"));
        self.shared.save()?;
        Ok(r)
    }

    // -- Jobs ---------------------------------------------------------------------------

    /// Sends files and folders: live if the target is online, otherwise queued until it
    /// shows up ("send ahead"). Until the transport exists every job is queued.
    pub fn send(&self, req: SendRequest) -> Result<JobId> {
        self.queue_send(req)
    }

    /// Queues a job for a known device. It is offered automatically the next time the
    /// device authenticates while this node runs (`docs/deferred-send.md`).
    ///
    /// Only metadata is collected here; files are not read or hashed until they are sent.
    pub fn queue_send(&self, req: SendRequest) -> Result<JobId> {
        if self.store.device(&req.target)?.is_none() {
            return Err(Error::UnknownDevice(req.target));
        }
        let mut scanned = manifest::scan(&req.sources)?;
        let id = JobId::random();
        let storage = match &self.config.outbox_storage {
            OutboxStorage::Reference => SourceStorage::Reference,
            OutboxStorage::CopyToCache(base) => {
                let dir = base.join(id.to_string());
                manifest::copy_to_cache(&mut scanned.entries, &dir)?;
                SourceStorage::Cached { dir }
            }
        };
        let job = Job {
            id,
            peer: req.target,
            direction: Direction::Outgoing,
            name: scanned.name,
            destination: req.destination,
            deferred: true,
            entries: scanned.entries,
            storage,
            total_bytes: scanned.total_bytes,
            bytes_done: 0,
            position: 0,
            created_unix: now_unix(),
            expires_unix: None,
            state: JobState::Waiting,
        };
        self.store.insert_job(job)?;
        let job = self.store.job(&id)?.ok_or(Error::NotFound)?;
        tracing::info!(%id, target = %req.target, files = job.entries.len(), "job queued");
        self.emit(NodeEvent::JobAdded {
            job: JobView::from(&job),
        });
        Ok(id)
    }

    /// All jobs in both directions, active ones in queue order first, then history.
    pub fn jobs(&self) -> Result<Vec<JobView>> {
        let mut jobs = self.store.jobs()?;
        jobs.sort_by_key(|j| (!j.state.is_active(), j.position, j.created_unix));
        Ok(jobs.iter().map(JobView::from).collect())
    }

    /// Applies an action from a job's ⋮ menu.
    pub fn job_action(&self, id: JobId, action: JobAction) -> Result<()> {
        let job = self.store.job(&id)?.ok_or(Error::NotFound)?;
        let new_state = match (action, job.state) {
            (JobAction::Pause, s) if s.is_active() && s != JobState::Paused => {
                Some(JobState::Paused)
            }
            (JobAction::Resume, JobState::Paused) => Some(JobState::Waiting),
            (JobAction::MoveToTop, s) if s.is_active() => {
                self.store.move_to_top(&id)?;
                None
            }
            (JobAction::Cancel, s) if s.is_active() || s == JobState::NoAnswer => {
                Some(JobState::Cancelled)
            }
            (JobAction::Clear, s) if !s.is_active() => {
                self.store.remove_job(&id)?;
                self.emit(NodeEvent::JobRemoved { id });
                return Ok(());
            }
            (JobAction::RetryFailed | JobAction::OfferAgain, _) => {
                return Err(Error::NotImplemented(
                    "retry and offer again need the engine",
                ));
            }
            _ => return Err(Error::InvalidState),
        };
        if let Some(state) = new_state {
            self.store.set_state(&id, state)?;
            if state == JobState::Cancelled {
                drop_cache(&job);
            }
            self.emit(NodeEvent::JobStateChanged { id, state });
        }
        Ok(())
    }

    /// Answers an incoming request (accept policy `Ask`). (M3)
    pub fn answer_request(&self, _id: JobId, _answer: AcceptAnswer) -> Result<()> {
        Err(Error::NotImplemented("incoming requests"))
    }

    /// Answers a conflict dialog; `None` cancels the job. (M4)
    pub fn answer_conflict(&self, _id: JobId, _mode: Option<ConflictMode>) -> Result<()> {
        Err(Error::NotImplemented("conflicts"))
    }

    /// Answers the "files changed" question for a queued job. (M5)
    pub fn answer_source_changed(&self, _id: JobId, _choice: SourceChangedChoice) -> Result<()> {
        Err(Error::NotImplemented("source changed"))
    }

    // -- Remote shares (M4) -------------------------------------------------------------

    /// Shares of an online peer that this device may see.
    pub async fn remote_shares(&self, _peer: DeviceId) -> Result<Vec<ShareInfo>> {
        Err(Error::NotImplemented("remote shares"))
    }

    /// One page of a folder in a peer's share.
    pub async fn list_dir(
        &self,
        _peer: DeviceId,
        _share: ShareId,
        _path: String,
        _cursor: Option<Cursor>,
    ) -> Result<(Vec<DirEntry>, Option<Cursor>)> {
        Err(Error::NotImplemented("list dir"))
    }

    /// Name search in a peer's shares (empty `shares` = all).
    pub async fn search(
        &self,
        _peer: DeviceId,
        _shares: Vec<ShareId>,
        _query: String,
        _cursor: Option<Cursor>,
    ) -> Result<(Vec<SearchHit>, Option<Cursor>)> {
        Err(Error::NotImplemented("search"))
    }

    /// Downloads files or folders from a peer's share into `to`.
    pub async fn download(
        &self,
        _peer: DeviceId,
        _share: ShareId,
        _paths: Vec<String>,
        _to: PathBuf,
    ) -> Result<JobId> {
        Err(Error::NotImplemented("download"))
    }

    /// Deletes in a peer's share; returns per-path errors (`None` = deleted).
    pub async fn delete_remote(
        &self,
        _peer: DeviceId,
        _share: ShareId,
        _paths: Vec<String>,
    ) -> Result<Vec<(String, Option<String>)>> {
        Err(Error::NotImplemented("remote delete"))
    }

    /// Creates a folder in a peer's share.
    pub async fn make_dir(&self, _peer: DeviceId, _share: ShareId, _path: String) -> Result<()> {
        Err(Error::NotImplemented("make dir"))
    }

    fn emit(&self, event: NodeEvent) {
        // A closed channel only means nobody listens (e.g. the CLI); not an error.
        let _ = self.events_tx.send(event);
    }
}

fn drop_cache(job: &Job) {
    if let SourceStorage::Cached { dir } = &job.storage {
        if let Err(e) = std::fs::remove_dir_all(dir) {
            tracing::warn!(id = %job.id, error = %e, "could not delete cached copies");
        }
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
    use localdrop_core::KdfParams;
    use localdrop_store::{KnownDevice, KnownDevices, MemoryStore};

    use super::*;

    fn node(
        store: Arc<MemoryStore>,
        storage: OutboxStorage,
    ) -> (Node, mpsc::UnboundedReceiver<NodeEvent>) {
        let group = GroupConfig::new("home").unwrap();
        let key = GroupKey::derive(&group, "pw", KdfParams::INSECURE_TEST).unwrap();
        let mut config = NodeConfig::new("test");
        config.outbox_storage = storage;
        Node::new(config, DeviceIdentity::generate(), group, key, store)
    }

    fn known(store: &MemoryStore) -> DeviceId {
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
        pk.device_id()
    }

    fn req(target: DeviceId, sources: Vec<PathBuf>) -> SendRequest {
        SendRequest {
            target,
            sources,
            destination: Destination::Inbox,
        }
    }

    #[test]
    fn queue_requires_known_device() {
        let (n, _) = node(Arc::new(MemoryStore::new()), OutboxStorage::Reference);
        let err = n.queue_send(req(DeviceId([1; 16]), vec![])).unwrap_err();
        assert!(matches!(err, Error::UnknownDevice(_)));
    }

    #[test]
    fn queue_folder_by_reference() {
        let store = Arc::new(MemoryStore::new());
        let target = known(&store);
        let (n, mut events) = node(store, OutboxStorage::Reference);
        let dir = manifest::tests::tempdir();
        std::fs::create_dir_all(dir.join("Photos/2024")).unwrap();
        std::fs::write(dir.join("Photos/2024/a.jpg"), b"hello").unwrap();

        let id = n.send(req(target, vec![dir.join("Photos")])).unwrap();
        let jobs = n.jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        let j = &jobs[0];
        assert_eq!((j.id, j.name.as_str()), (id, "Photos"));
        assert_eq!((j.files_total, j.bytes_total), (1, 5));
        assert_eq!(j.state, JobState::Waiting);
        assert!(j.deferred);
        assert!(matches!(events.try_recv(), Ok(NodeEvent::JobAdded { .. })));
        assert_eq!(n.devices().unwrap()[0].queued_jobs, 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cached_copies_are_dropped_on_cancel() {
        let store = Arc::new(MemoryStore::new());
        let target = known(&store);
        let dir = manifest::tests::tempdir();
        let (n, _) = node(store, OutboxStorage::CopyToCache(dir.join("cache")));
        std::fs::write(dir.join("f.txt"), b"x").unwrap();

        let id = n.queue_send(req(target, vec![dir.join("f.txt")])).unwrap();
        let cached = dir.join("cache").join(id.to_string()).join("f.txt");
        assert!(cached.exists());
        n.job_action(id, JobAction::Cancel).unwrap();
        assert!(!cached.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn job_actions_follow_states() {
        let store = Arc::new(MemoryStore::new());
        let target = known(&store);
        let (n, _) = node(store, OutboxStorage::Reference);
        let dir = manifest::tests::tempdir();
        std::fs::write(dir.join("a"), b"a").unwrap();
        std::fs::write(dir.join("b"), b"b").unwrap();
        let a = n.queue_send(req(target, vec![dir.join("a")])).unwrap();
        let b = n.queue_send(req(target, vec![dir.join("b")])).unwrap();

        n.job_action(b, JobAction::MoveToTop).unwrap();
        assert_eq!(n.jobs().unwrap()[0].id, b);

        n.job_action(a, JobAction::Pause).unwrap();
        assert!(matches!(
            n.job_action(a, JobAction::Pause),
            Err(Error::InvalidState)
        ));
        n.job_action(a, JobAction::Resume).unwrap();
        assert!(matches!(
            n.job_action(a, JobAction::Clear),
            Err(Error::InvalidState)
        ));
        n.job_action(a, JobAction::Cancel).unwrap();
        n.job_action(a, JobAction::Clear).unwrap();
        assert_eq!(n.jobs().unwrap().len(), 1);

        n.forget_device(target).unwrap();
        assert_eq!(n.jobs().unwrap()[0].state, JobState::Cancelled);
        assert!(n.devices().unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn group_info_and_accept_override() {
        let store = Arc::new(MemoryStore::new());
        let target = known(&store);
        let (n, _) = node(store, OutboxStorage::Reference);
        assert_eq!(n.group_info().name, "home");
        assert_eq!(n.group_info().fingerprint.len(), 9);
        n.set_device_accept(target, Some(AcceptPolicy::Ask))
            .unwrap();
        assert_eq!(
            n.devices().unwrap()[0].accept_override,
            Some(AcceptPolicy::Ask)
        );
        assert!(n.set_device_accept(DeviceId([7; 16]), None).is_err());
    }
}
