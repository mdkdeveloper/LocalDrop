//! Incoming-transfer accept policy.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::DeviceId;

/// What to do with an incoming offer from a group member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AcceptPolicy {
    /// Accept and start receiving immediately. Required for fully unattended
    /// deferred delivery.
    #[default]
    AutoAccept,
    /// Hold the offer until the user explicitly accepts or rejects it.
    Ask,
}

/// Group-wide default policy plus optional per-device overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptSettings {
    /// Policy for devices without an override.
    pub default: AcceptPolicy,
    /// Per-device overrides.
    pub overrides: HashMap<DeviceId, AcceptPolicy>,
}

impl AcceptSettings {
    /// Effective policy for the given sender.
    pub fn policy_for(&self, device: &DeviceId) -> AcceptPolicy {
        self.overrides.get(device).copied().unwrap_or(self.default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        let dev = DeviceId([1; 16]);
        let other = DeviceId([2; 16]);
        let mut s = AcceptSettings::default();
        s.overrides.insert(dev, AcceptPolicy::Ask);
        assert_eq!(s.policy_for(&dev), AcceptPolicy::Ask);
        assert_eq!(s.policy_for(&other), AcceptPolicy::AutoAccept);
    }
}
