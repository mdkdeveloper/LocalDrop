//! Scripted events for building and testing the UI without a network
//! (feature `fake`, see `docs/node-api.md#fake-node`).
//!
//! A [`Script`] is a list of events with delays. [`demo`] tells a short story that
//! touches most screens of the design: devices appear, a job is sent with progress, a
//! request and a conflict need answers, the receiver runs low on space, a device goes
//! offline and a queued job waits for it.

use std::time::Duration;

use localdrop_core::{DeviceId, JobId};
use localdrop_protocol::ConflictMode;
use localdrop_store::{Direction, JobState};
use tokio::sync::mpsc;

use crate::NodeEvent;
use crate::api::{Destination, JobView};

/// One scripted event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// Delay before this event, counted from the previous one.
    pub after: Duration,
    /// The event.
    pub event: NodeEvent,
}

/// A sequence of events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Script {
    /// Steps in order.
    pub steps: Vec<Step>,
}

impl Script {
    /// Appends an event after `ms` milliseconds.
    pub fn then(mut self, ms: u64, event: NodeEvent) -> Self {
        self.steps.push(Step {
            after: Duration::from_millis(ms),
            event,
        });
        self
    }

    /// Plays the script into a new channel. `speed` > 1 plays faster; 0 = no delays.
    pub fn play(self, speed: u32) -> mpsc::UnboundedReceiver<NodeEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            for step in self.steps {
                if speed > 0 && !step.after.is_zero() {
                    tokio::time::sleep(step.after / speed).await;
                }
                if tx.send(step.event).is_err() {
                    break;
                }
            }
        });
        rx
    }
}

/// Stable ids for the demo devices, so UI tests can refer to them.
pub mod ids {
    use super::*;

    /// "Work laptop" (desktop, online).
    pub const LAPTOP: DeviceId = DeviceId([0x11; 16]);
    /// "Pixel 8" (phone, goes offline).
    pub const PHONE: DeviceId = DeviceId([0x22; 16]);
    /// "Living room PC" (known, offline the whole time).
    pub const DESKTOP: DeviceId = DeviceId([0x33; 16]);
    /// Outgoing job "Holiday photos" to the laptop.
    pub const PHOTOS: JobId = JobId([0xa1; 16]);
    /// Incoming request "Scans" from the phone.
    pub const SCANS: JobId = JobId([0xa2; 16]);
    /// Queued job "Project files" for the offline desktop.
    pub const PROJECT: JobId = JobId([0xa3; 16]);
}

fn job(
    id: JobId,
    peer: DeviceId,
    direction: Direction,
    name: &str,
    files: u32,
    bytes: u64,
) -> JobView {
    JobView {
        id,
        peer,
        direction,
        name: name.into(),
        destination: Destination::Inbox,
        deferred: false,
        files_total: files,
        bytes_total: bytes,
        bytes_done: 0,
        state: JobState::Waiting,
        created_unix: 1_763_200_000,
    }
}

/// The demo story.
pub fn demo() -> Script {
    use ids::*;
    const GB: u64 = 1_000_000_000;
    let photos_total = 3_200_000_000;
    let mut s = Script::default()
        .then(
            0,
            NodeEvent::DiscoveryChanged {
                on: true,
                active_networks: 1,
            },
        )
        .then(800, NodeEvent::PeerFound { device: LAPTOP })
        .then(
            300,
            NodeEvent::PeerOnline {
                device: LAPTOP,
                name: "Work laptop".into(),
            },
        )
        .then(
            600,
            NodeEvent::PeerOnline {
                device: PHONE,
                name: "Pixel 8".into(),
            },
        )
        .then(
            1000,
            NodeEvent::JobAdded {
                job: job(
                    PHOTOS,
                    LAPTOP,
                    Direction::Outgoing,
                    "Holiday photos",
                    1820,
                    photos_total,
                ),
            },
        )
        .then(
            200,
            NodeEvent::JobStateChanged {
                id: PHOTOS,
                state: JobState::InProgress,
            },
        );
    for i in 1..=8u64 {
        s = s.then(
            250,
            NodeEvent::JobProgress {
                id: PHOTOS,
                bytes_done: photos_total * i / 20,
                bytes_total: photos_total,
                speed_bps: 11_400_000,
            },
        );
    }
    let mut scans = job(SCANS, PHONE, Direction::Incoming, "Scans", 12, 48_000_000);
    scans.state = JobState::AwaitingAccept;
    let mut project = job(
        PROJECT,
        DESKTOP,
        Direction::Outgoing,
        "Project files",
        340,
        2 * GB,
    );
    project.deferred = true;
    s.then(500, NodeEvent::JobAdded { job: scans })
        .then(
            0,
            NodeEvent::IncomingRequest {
                id: SCANS,
                from: PHONE,
                name: "Scans".into(),
                files: 12,
                bytes: 48_000_000,
                destination: Destination::Inbox,
            },
        )
        .then(3000, NodeEvent::DecisionMade { id: SCANS })
        .then(
            0,
            NodeEvent::JobStateChanged {
                id: SCANS,
                state: JobState::AwaitingConflictDecision,
            },
        )
        .then(
            0,
            NodeEvent::ConflictAsked {
                id: SCANS,
                conflicts: 3,
                allowed_modes: vec![
                    ConflictMode::KeepBoth,
                    ConflictMode::KeepExisting,
                    ConflictMode::ReplaceWithBackup,
                ],
            },
        )
        .then(3000, NodeEvent::DecisionMade { id: SCANS })
        .then(
            0,
            NodeEvent::JobStateChanged {
                id: SCANS,
                state: JobState::Completed,
            },
        )
        .then(
            500,
            NodeEvent::LowSpace {
                id: PHOTOS,
                free: 1_200_000_000,
                needed: 3_400_000_000,
            },
        )
        .then(
            0,
            NodeEvent::JobStateChanged {
                id: PHOTOS,
                state: JobState::LowSpace,
            },
        )
        .then(2000, NodeEvent::PeerOffline { device: PHONE })
        .then(500, NodeEvent::JobAdded { job: project })
        .then(
            1000,
            NodeEvent::UntrustedNetwork {
                key: "5f0c9a1e".into(),
                label: "Office-5G".into(),
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn demo_plays_in_order() {
        let script = demo();
        let expected = script.steps.len();
        let mut rx = script.play(0);
        let mut got = Vec::new();
        while let Some(e) = rx.recv().await {
            got.push(e);
        }
        assert_eq!(got.len(), expected);
        assert!(matches!(
            got[0],
            NodeEvent::DiscoveryChanged { on: true, .. }
        ));
        assert!(matches!(
            got.last(),
            Some(NodeEvent::UntrustedNetwork { .. })
        ));
    }
}
