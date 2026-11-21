//! What the Flutter app calls (docs/node-api.md). `flutter_rust_bridge_codegen generate`
//! turns this module into `app/lib/src/rust/api/node.dart`.
//!
//! The node needs a tokio runtime; FRB's own executor is not one, so every call runs on
//! [`RT`]. Types crossing the bridge are flat DTOs with hex-string ids and plain enums
//! (no enums with fields, which would need `freezed` on the Dart side).

use std::str::FromStr;
use std::sync::{Arc, LazyLock, Mutex};

use anyhow::{Context, Result, anyhow};
use flutter_rust_bridge::frb;
use localdrop_core::{DeviceId, JobId, KdfParams};
use localdrop_node::{
    self as ln, Destination, Direction, JobState, NetworkSelection, NetworkTrust, Node, NodeEvent,
    OnNewNetwork, Presence, StateDir,
};
use localdrop_store::MemoryStore;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::frb_generated::StreamSink;

static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("localdrop")
        .enable_all()
        .build()
        .expect("tokio runtime")
});

/// Runs `f` on the node's runtime and waits for it from any executor.
async fn on_rt<T: Send + 'static>(
    f: impl std::future::Future<Output = Result<T>> + Send + 'static,
) -> Result<T> {
    RT.spawn(f).await.context("node task failed")?
}

#[frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

// -- DTOs ---------------------------------------------------------------------------------

pub struct GroupInfoDto {
    pub name: String,
    pub fingerprint: String,
}

impl From<ln::GroupInfo> for GroupInfoDto {
    fn from(g: ln::GroupInfo) -> Self {
        Self {
            name: g.name,
            fingerprint: g.fingerprint,
        }
    }
}

pub enum PresenceDto {
    Online,
    Found,
    Offline,
}

pub enum AcceptPolicyDto {
    AutoAccept,
    Ask,
}

pub struct DeviceDto {
    pub id: String,
    pub name: String,
    pub presence: PresenceDto,
    pub last_seen_unix: u64,
    pub accept_override: Option<AcceptPolicyDto>,
    pub queued_jobs: u32,
}

pub enum DirectionDto {
    Outgoing,
    Incoming,
}

pub enum JobStateDto {
    Waiting,
    AwaitingAccept,
    AwaitingConflictDecision,
    LowSpace,
    InProgress,
    Paused,
    SourceChanged,
    NoAnswer,
    Completed,
    CompletedWithErrors,
    Rejected,
    Failed,
    Cancelled,
    Expired,
}

fn job_state(s: JobState) -> JobStateDto {
    match s {
        JobState::Waiting => JobStateDto::Waiting,
        JobState::AwaitingAccept => JobStateDto::AwaitingAccept,
        JobState::AwaitingConflictDecision => JobStateDto::AwaitingConflictDecision,
        JobState::LowSpace => JobStateDto::LowSpace,
        JobState::InProgress => JobStateDto::InProgress,
        JobState::Paused => JobStateDto::Paused,
        JobState::SourceChanged => JobStateDto::SourceChanged,
        JobState::NoAnswer => JobStateDto::NoAnswer,
        JobState::Completed => JobStateDto::Completed,
        JobState::CompletedWithErrors => JobStateDto::CompletedWithErrors,
        JobState::Rejected => JobStateDto::Rejected,
        JobState::Failed => JobStateDto::Failed,
        JobState::Cancelled => JobStateDto::Cancelled,
        JobState::Expired => JobStateDto::Expired,
    }
}

/// `Destination`: `share_id` empty = the inbox.
pub struct DestinationDto {
    pub share_id: String,
    pub path: String,
}

fn destination(d: &Destination) -> DestinationDto {
    match d {
        Destination::Inbox => DestinationDto {
            share_id: String::new(),
            path: String::new(),
        },
        Destination::Share { share_id, path } => DestinationDto {
            share_id: share_id.to_string(),
            path: path.clone(),
        },
    }
}

pub struct JobDto {
    pub id: String,
    pub peer: String,
    pub direction: DirectionDto,
    pub name: String,
    pub destination: DestinationDto,
    pub deferred: bool,
    pub files_total: u32,
    pub bytes_total: u64,
    pub bytes_done: u64,
    pub state: JobStateDto,
    pub created_unix: u64,
}

impl From<ln::JobView> for JobDto {
    fn from(j: ln::JobView) -> Self {
        Self {
            id: j.id.to_string(),
            peer: j.peer.to_string(),
            direction: match j.direction {
                Direction::Outgoing => DirectionDto::Outgoing,
                Direction::Incoming => DirectionDto::Incoming,
            },
            name: j.name,
            destination: destination(&j.destination),
            deferred: j.deferred,
            files_total: j.files_total,
            bytes_total: j.bytes_total,
            bytes_done: j.bytes_done,
            state: job_state(j.state),
            created_unix: j.created_unix,
        }
    }
}

pub enum NetworkTrustDto {
    Trusted,
    NotTrusted,
    NeverAsk,
}

pub enum NetworkKindDto {
    WiFi,
    Ethernet,
}

pub struct NetworkDto {
    pub key: String,
    pub label: String,
    pub kind: NetworkKindDto,
    pub address: Option<String>,
    pub last_seen_unix: u64,
    pub connected: bool,
    pub active: bool,
    pub trust: NetworkTrustDto,
    pub weak_identity: bool,
}

pub struct NetworkSettingsDto {
    pub discover_at_launch: bool,
    pub ask_on_new_network: bool,
    pub trust_every_network: bool,
    /// Network menu: `None` = every trusted network.
    pub only_network: Option<String>,
}

pub enum JobActionDto {
    Pause,
    Resume,
    MoveToTop,
    Cancel,
    RetryFailed,
    OfferAgain,
    Clear,
}

pub enum EventKind {
    DiscoveryChanged,
    UntrustedNetwork,
    PeerFound,
    PeerOnline,
    PeerOffline,
    KeyMismatch,
    JobAdded,
    JobStateChanged,
    JobProgress,
    JobRemoved,
    IncomingRequest,
    ConflictAsked,
    DecisionMade,
    LowSpace,
    Error,
    /// A newer event this app version doesn't know; ignore it.
    Other,
}

/// One `NodeEvent`, flattened: which fields are set depends on [`EventKind`].
pub struct EventDto {
    pub kind: EventKind,
    /// DiscoveryChanged.
    pub on: bool,
    /// DiscoveryChanged: active networks; IncomingRequest: files; ConflictAsked: conflicts.
    pub count: u32,
    /// Network key, device id or job id.
    pub id: String,
    /// Device id of the sender (IncomingRequest).
    pub from: String,
    /// Label, device name, job name or error message.
    pub text: String,
    pub job: Option<JobDto>,
    pub state: Option<JobStateDto>,
    /// JobProgress: done/total/speed; IncomingRequest: bytes in `a`;
    /// LowSpace: free in `a`, needed in `b`.
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub destination: Option<DestinationDto>,
}

impl EventDto {
    fn new(kind: EventKind) -> Self {
        Self {
            kind,
            on: false,
            count: 0,
            id: String::new(),
            from: String::new(),
            text: String::new(),
            job: None,
            state: None,
            a: 0,
            b: 0,
            c: 0,
            destination: None,
        }
    }
}

fn event(e: NodeEvent) -> EventDto {
    use EventKind as K;
    match e {
        NodeEvent::DiscoveryChanged {
            on,
            active_networks,
        } => EventDto {
            on,
            count: active_networks,
            ..EventDto::new(K::DiscoveryChanged)
        },
        NodeEvent::UntrustedNetwork { key, label } => EventDto {
            id: key,
            text: label,
            ..EventDto::new(K::UntrustedNetwork)
        },
        NodeEvent::PeerFound { device } => EventDto {
            id: device.to_string(),
            ..EventDto::new(K::PeerFound)
        },
        NodeEvent::PeerOnline { device, name } => EventDto {
            id: device.to_string(),
            text: name,
            ..EventDto::new(K::PeerOnline)
        },
        NodeEvent::PeerOffline { device } => EventDto {
            id: device.to_string(),
            ..EventDto::new(K::PeerOffline)
        },
        NodeEvent::KeyMismatch { device, name } => EventDto {
            id: device.to_string(),
            text: name,
            ..EventDto::new(K::KeyMismatch)
        },
        NodeEvent::JobAdded { job } => EventDto {
            id: job.id.to_string(),
            job: Some(job.into()),
            ..EventDto::new(K::JobAdded)
        },
        NodeEvent::JobStateChanged { id, state } => EventDto {
            id: id.to_string(),
            state: Some(job_state(state)),
            ..EventDto::new(K::JobStateChanged)
        },
        NodeEvent::JobProgress {
            id,
            bytes_done,
            bytes_total,
            speed_bps,
        } => EventDto {
            id: id.to_string(),
            a: bytes_done,
            b: bytes_total,
            c: speed_bps,
            ..EventDto::new(K::JobProgress)
        },
        NodeEvent::JobRemoved { id } => EventDto {
            id: id.to_string(),
            ..EventDto::new(K::JobRemoved)
        },
        NodeEvent::IncomingRequest {
            id,
            from,
            name,
            files,
            bytes,
            destination: d,
        } => EventDto {
            id: id.to_string(),
            from: from.to_string(),
            text: name,
            count: files,
            a: bytes,
            destination: Some(destination(&d)),
            ..EventDto::new(K::IncomingRequest)
        },
        NodeEvent::ConflictAsked { id, conflicts, .. } => EventDto {
            id: id.to_string(),
            count: conflicts,
            ..EventDto::new(K::ConflictAsked)
        },
        NodeEvent::DecisionMade { id } => EventDto {
            id: id.to_string(),
            ..EventDto::new(K::DecisionMade)
        },
        NodeEvent::LowSpace { id, free, needed } => EventDto {
            id: id.to_string(),
            a: free,
            b: needed,
            ..EventDto::new(K::LowSpace)
        },
        NodeEvent::Error { message } => EventDto {
            text: message,
            ..EventDto::new(K::Error)
        },
        _ => EventDto::new(K::Other),
    }
}

fn device_id(s: &str) -> Result<DeviceId> {
    DeviceId::from_str(s).map_err(|_| anyhow!("bad device id {s}"))
}

fn job_id(s: &str) -> Result<JobId> {
    s.parse().map_err(|_| anyhow!("bad job id {s}"))
}

// -- Joining ------------------------------------------------------------------------------

/// The per-user state folder (`$LOCALDROP_HOME` or the platform default).
#[frb(sync)]
pub fn default_home() -> String {
    StateDir::default_location().to_string_lossy().into_owned()
}

/// Whether this device has joined a group in `home`.
#[frb(sync)]
pub fn is_joined(home: String) -> bool {
    StateDir::new(home).is_joined()
}

/// The join form: derives the key (Argon2id, ~0.5 s) and saves the state.
pub async fn join_group(
    home: String,
    group_name: String,
    password: String,
    device_name: String,
    trust_this_network: bool,
) -> Result<GroupInfoDto> {
    on_rt(async move {
        tokio::task::spawn_blocking(move || {
            ln::join_group(
                &StateDir::new(home),
                &group_name,
                &password,
                &device_name,
                trust_this_network,
                KdfParams::V1,
            )
            .map(|(g, _)| g.into())
            .map_err(anyhow::Error::from)
        })
        .await?
    })
    .await
}

// -- The node -----------------------------------------------------------------------------

/// A node opened from a joined state folder.
#[frb(opaque)]
pub struct LdNode {
    node: Arc<Node>,
    events: Mutex<Option<UnboundedReceiver<NodeEvent>>>,
}

impl LdNode {
    /// Opens the node of a joined state folder (not started yet).
    pub async fn open(home: String) -> Result<LdNode> {
        // Node::open spawns nothing; it only needs the runtime later for `start`.
        let (node, rx) = Node::open(StateDir::new(home), Arc::new(MemoryStore::new()))?;
        Ok(LdNode {
            node: Arc::new(node),
            events: Mutex::new(Some(rx)),
        })
    }

    /// All node events, in order. Can be called once.
    pub fn events(&self, sink: StreamSink<EventDto>) -> Result<()> {
        let mut rx = self
            .events
            .lock()
            .map_err(|_| anyhow!("poisoned"))?
            .take()
            .ok_or_else(|| anyhow!("events already taken"))?;
        RT.spawn(async move {
            while let Some(e) = rx.recv().await {
                if sink.add(event(e)).is_err() {
                    break;
                }
            }
        });
        Ok(())
    }

    #[frb(sync)]
    pub fn device_id(&self) -> String {
        self.node.device_id().to_string()
    }

    #[frb(sync)]
    pub fn device_name(&self) -> String {
        self.node.settings().device_name
    }

    #[frb(sync)]
    pub fn group_info(&self) -> GroupInfoDto {
        self.node.group_info().into()
    }

    pub async fn start(&self) -> Result<()> {
        let n = self.node.clone();
        on_rt(async move { Ok(n.start().await?) }).await
    }

    pub async fn stop(&self) -> Result<()> {
        let n = self.node.clone();
        on_rt(async move { Ok(n.stop().await?) }).await
    }

    #[frb(sync)]
    pub fn discovery_on(&self) -> bool {
        self.node.discovery_on()
    }

    pub async fn set_discovery(&self, on: bool) -> Result<()> {
        let n = self.node.clone();
        on_rt(async move { Ok(n.set_discovery(on).await?) }).await
    }

    pub async fn change_password(&self, current: String, new: String) -> Result<GroupInfoDto> {
        let n = self.node.clone();
        on_rt(async move { Ok(n.change_password(current, new).await?.into()) }).await
    }

    pub async fn leave_group(&self, keep_queue: bool, forget_devices: bool) -> Result<()> {
        let n = self.node.clone();
        on_rt(async move { Ok(n.leave_group(keep_queue, forget_devices).await?) }).await
    }

    // -- devices --

    #[frb(sync)]
    pub fn devices(&self) -> Result<Vec<DeviceDto>> {
        Ok(self
            .node
            .devices()?
            .into_iter()
            .map(|d| DeviceDto {
                id: d.id.to_string(),
                name: d.name,
                presence: match d.presence {
                    Presence::Online => PresenceDto::Online,
                    Presence::Found => PresenceDto::Found,
                    Presence::Offline => PresenceDto::Offline,
                },
                last_seen_unix: d.last_seen_unix,
                accept_override: d.accept_override.map(|p| match p {
                    localdrop_core::AcceptPolicy::AutoAccept => AcceptPolicyDto::AutoAccept,
                    localdrop_core::AcceptPolicy::Ask => AcceptPolicyDto::Ask,
                }),
                queued_jobs: d.queued_jobs,
            })
            .collect())
    }

    /// *Connect by address*; returns the device id that answered.
    pub async fn connect_address(&self, address: String, remember: bool) -> Result<String> {
        let n = self.node.clone();
        on_rt(async move { Ok(n.connect_address(address, remember).await?.to_string()) }).await
    }

    /// Round trip in milliseconds.
    pub async fn ping(&self, device: String) -> Result<f64> {
        let n = self.node.clone();
        let id = device_id(&device)?;
        on_rt(async move { Ok(n.ping(id).await?.as_secs_f64() * 1000.0) }).await
    }

    // -- networks --

    #[frb(sync)]
    pub fn networks(&self) -> Result<Vec<NetworkDto>> {
        Ok(self
            .node
            .networks()?
            .into_iter()
            .map(|n| NetworkDto {
                key: n.key,
                label: n.label,
                kind: match n.kind {
                    ln::api::NetworkKind::WiFi => NetworkKindDto::WiFi,
                    ln::api::NetworkKind::Ethernet => NetworkKindDto::Ethernet,
                },
                address: n.address,
                last_seen_unix: n.last_seen_unix,
                connected: n.connected,
                active: n.active,
                trust: match n.trust {
                    NetworkTrust::Trusted => NetworkTrustDto::Trusted,
                    NetworkTrust::NotTrusted => NetworkTrustDto::NotTrusted,
                    NetworkTrust::NeverAsk => NetworkTrustDto::NeverAsk,
                },
                weak_identity: n.weak_identity,
            })
            .collect())
    }

    pub async fn set_network_trust(&self, key: String, trust: NetworkTrustDto) -> Result<()> {
        let n = self.node.clone();
        let t = match trust {
            NetworkTrustDto::Trusted => NetworkTrust::Trusted,
            NetworkTrustDto::NotTrusted => NetworkTrust::NotTrusted,
            NetworkTrustDto::NeverAsk => NetworkTrust::NeverAsk,
        };
        on_rt(async move { Ok(n.set_network_trust(key, t).await?) }).await
    }

    #[frb(sync)]
    pub fn rename_network(&self, key: String, label: String) -> Result<()> {
        Ok(self.node.rename_network(key, label)?)
    }

    #[frb(sync)]
    pub fn forget_network(&self, key: String) -> Result<()> {
        Ok(self.node.forget_network(key)?)
    }

    #[frb(sync)]
    pub fn network_settings(&self) -> NetworkSettingsDto {
        let s = self.node.network_settings();
        NetworkSettingsDto {
            discover_at_launch: s.discover_at_launch,
            ask_on_new_network: s.on_new_network == OnNewNetwork::AskToTrust,
            trust_every_network: s.trust_every_network,
            only_network: match s.selection {
                NetworkSelection::TrustedNetworks => None,
                NetworkSelection::Only(k) => Some(k.0.iter().map(|b| format!("{b:02x}")).collect()),
            },
        }
    }

    pub async fn set_network_settings(&self, settings: NetworkSettingsDto) -> Result<()> {
        let n = self.node.clone();
        // The menu selection refers to a connected network's full key.
        let only = match &settings.only_network {
            None => None,
            Some(k) => Some(
                n.networks()?
                    .into_iter()
                    .find(|v| v.key.starts_with(k.as_str()))
                    .map(|v| v.key)
                    .ok_or_else(|| anyhow!("no such network"))?,
            ),
        };
        on_rt(async move {
            n.set_network_settings(move |s| {
                s.discover_at_launch = settings.discover_at_launch;
                s.on_new_network = if settings.ask_on_new_network {
                    OnNewNetwork::AskToTrust
                } else {
                    OnNewNetwork::StaySilent
                };
                s.trust_every_network = settings.trust_every_network;
                s.selection = match only.as_deref().and_then(parse_network_key) {
                    Some(k) => NetworkSelection::Only(k),
                    None => NetworkSelection::TrustedNetworks,
                };
            })
            .await?;
            Ok(())
        })
        .await
    }

    // -- jobs --

    #[frb(sync)]
    pub fn jobs(&self) -> Result<Vec<JobDto>> {
        Ok(self.node.jobs()?.into_iter().map(Into::into).collect())
    }

    #[frb(sync)]
    pub fn job_action(&self, id: String, action: JobActionDto) -> Result<()> {
        let a = match action {
            JobActionDto::Pause => ln::JobAction::Pause,
            JobActionDto::Resume => ln::JobAction::Resume,
            JobActionDto::MoveToTop => ln::JobAction::MoveToTop,
            JobActionDto::Cancel => ln::JobAction::Cancel,
            JobActionDto::RetryFailed => ln::JobAction::RetryFailed,
            JobActionDto::OfferAgain => ln::JobAction::OfferAgain,
            JobActionDto::Clear => ln::JobAction::Clear,
        };
        Ok(self.node.job_action(job_id(&id)?, a)?)
    }

    /// Accept / decline an incoming job (needs the transfer engine, M3).
    #[frb(sync)]
    pub fn answer_request(&self, id: String, accept: bool, always: bool) -> Result<()> {
        Ok(self
            .node
            .answer_request(job_id(&id)?, ln::AcceptAnswer { accept, always })?)
    }
}

fn parse_network_key(hex: &str) -> Option<localdrop_node::NetworkKey> {
    if hex.len() != 64 {
        return None;
    }
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(localdrop_node::NetworkKey(k))
}
