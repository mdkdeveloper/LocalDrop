//! Persistent settings and the state directory (desktop / CLI).
//!
//! On desktop the node keeps its state in one folder:
//!
//! | File | Content |
//! |---|---|
//! | `identity.key` | 32-byte Ed25519 secret of this device |
//! | `group.key` | derived group keys (`postcard`); the password is never stored |
//! | `settings.json` | [`Settings`] |
//!
//! Both key files are written with mode `0600` on Unix. On mobile and in the desktop
//! app the keys move to platform secure storage (docs/security.md#keys), which the app
//! owns and hands to the node.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use localdrop_core::{DeviceIdentity, Error, GroupConfig, GroupKey, KdfParams, Result};
use localdrop_discovery::{NetworkKey, NetworkKind};
use serde::{Deserialize, Serialize};

use crate::api::NetworkTrust;

/// Which trusted networks discovery uses (the network menu).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NetworkSelection {
    /// Every connected trusted network.
    #[default]
    TrustedNetworks,
    /// Only this one.
    Only(NetworkKey),
}

/// What to do when connected to a network that isn't trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OnNewNetwork {
    /// Ask once per network ("Trust “Office-5G”?").
    #[default]
    AskToTrust,
    /// Ask nothing; trust from the network menu or settings.
    StaySilent,
}

/// *Settings › Networks* (docs/networks.md#trusted-networks).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkSettings {
    /// Every Wi-Fi/Ethernet network counts as trusted ("not recommended").
    pub trust_every_network: bool,
    /// Network menu selection.
    pub selection: NetworkSelection,
    /// Whether the discovery switch starts on.
    pub discover_at_launch: bool,
    /// Ask to trust new networks.
    pub on_new_network: OnNewNetwork,
    /// Virtual or VPN interfaces (by OS name) added by hand.
    pub include_interfaces: Vec<String>,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            trust_every_network: false,
            selection: NetworkSelection::TrustedNetworks,
            discover_at_launch: true,
            on_new_network: OnNewNetwork::AskToTrust,
            include_interfaces: Vec::new(),
        }
    }
}

/// A network this device has seen and the user decided about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownNetwork {
    /// Fingerprint.
    pub key: NetworkKey,
    /// User-editable label (defaults to the Wi-Fi name or "Ethernet 192.168.1.x").
    pub label: String,
    /// Wi-Fi or Ethernet.
    pub kind: NetworkKind,
    /// Trust decision.
    pub trust: NetworkTrust,
    /// Known only by subnet and gateway IP.
    pub weak: bool,
    /// Unix seconds.
    pub last_seen_unix: u64,
}

/// Everything persisted besides the keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Name other devices see.
    pub device_name: String,
    /// Normalized group name.
    pub group_name: String,
    /// QUIC port; 0 = random at every start (default, avoids linking announcements).
    pub listen_port: u16,
    /// *Visible to your group*: announce, or only browse.
    pub visible: bool,
    /// Network settings.
    pub networks: NetworkSettings,
    /// Remembered networks.
    pub known_networks: Vec<KnownNetwork>,
}

impl Settings {
    /// Defaults for a device that just joined `group_name`.
    pub fn new(device_name: impl Into<String>, group_name: impl Into<String>) -> Self {
        Self {
            device_name: device_name.into(),
            group_name: group_name.into(),
            listen_port: 0,
            visible: true,
            networks: NetworkSettings::default(),
            known_networks: Vec::new(),
        }
    }

    /// The remembered network with this key.
    pub fn network(&self, key: &NetworkKey) -> Option<&KnownNetwork> {
        self.known_networks.iter().find(|n| &n.key == key)
    }

    /// Records a trust decision, adding the network if it's new.
    pub fn set_trust(
        &mut self,
        key: NetworkKey,
        label: &str,
        kind: NetworkKind,
        weak: bool,
        trust: NetworkTrust,
    ) {
        let now = now_unix();
        match self.known_networks.iter_mut().find(|n| n.key == key) {
            Some(n) => {
                n.trust = trust;
                n.last_seen_unix = now;
            }
            None => self.known_networks.push(KnownNetwork {
                key,
                label: label.to_owned(),
                kind,
                trust,
                weak,
                last_seen_unix: now,
            }),
        }
    }
}

/// What [`StateDir::load`] returns.
#[derive(Debug)]
pub struct Joined {
    /// This device's key.
    pub identity: DeviceIdentity,
    /// Group name.
    pub group: GroupConfig,
    /// Group key.
    pub key: GroupKey,
    /// Settings.
    pub settings: Settings,
}

/// The folder holding this device's state.
#[derive(Debug, Clone)]
pub struct StateDir {
    root: PathBuf,
}

const IDENTITY: &str = "identity.key";
const GROUP: &str = "group.key";
const SETTINGS: &str = "settings.json";

impl StateDir {
    /// `$LOCALDROP_HOME`, or the platform's per-user config folder:
    /// `%APPDATA%\LocalDrop`, `~/Library/Application Support/LocalDrop`,
    /// `$XDG_CONFIG_HOME/localdrop` (or `~/.config/localdrop`).
    pub fn default_location() -> PathBuf {
        if let Some(p) = std::env::var_os("LOCALDROP_HOME") {
            return PathBuf::from(p);
        }
        let home = || {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| ".".into())
        };
        if cfg!(windows) {
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(home)
                .join("LocalDrop")
        } else if cfg!(target_os = "macos") {
            home().join("Library/Application Support/LocalDrop")
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join(".config"))
                .join("localdrop")
        }
    }

    /// A state folder at `root` (created on [`init`](Self::init)).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The folder.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Whether this device has joined a group.
    pub fn is_joined(&self) -> bool {
        self.root.join(GROUP).exists() && self.root.join(SETTINGS).exists()
    }

    /// Joins (or re-joins) a group: derives the key, keeps the existing device identity
    /// or creates one, and writes everything. The password is not stored.
    pub fn init(
        &self,
        group_name: &str,
        password: &str,
        settings: impl FnOnce(&GroupConfig) -> Settings,
        kdf: KdfParams,
    ) -> Result<Joined> {
        let group = GroupConfig::new(group_name)?;
        let key = GroupKey::derive(&group, password, kdf)?;
        std::fs::create_dir_all(&self.root)?;
        let identity = match self.read_identity() {
            Ok(id) => id,
            Err(_) => {
                let id = DeviceIdentity::generate();
                write_private(&self.root.join(IDENTITY), &id.secret_bytes())?;
                id
            }
        };
        let bytes = postcard::to_stdvec(&key).map_err(|e| Error::Protocol(e.to_string()))?;
        write_private(&self.root.join(GROUP), &bytes)?;
        let settings = settings(&group);
        self.save_settings(&settings)?;
        Ok(Joined {
            identity,
            group,
            key,
            settings,
        })
    }

    /// Loads a joined state.
    pub fn load(&self) -> Result<Joined> {
        let identity = self.read_identity()?;
        let key: GroupKey = postcard::from_bytes(&std::fs::read(self.root.join(GROUP))?)
            .map_err(|e| Error::Protocol(format!("group.key: {e}")))?;
        let settings: Settings = serde_json::from_slice(&std::fs::read(self.root.join(SETTINGS))?)
            .map_err(|e| Error::Protocol(format!("settings.json: {e}")))?;
        let group = GroupConfig::new(&settings.group_name)?;
        Ok(Joined {
            identity,
            group,
            key,
            settings,
        })
    }

    /// Writes the settings (atomically: temp file, then rename).
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let json =
            serde_json::to_vec_pretty(settings).map_err(|e| Error::Protocol(e.to_string()))?;
        write_atomic(&self.root.join(SETTINGS), &json, false)
    }

    /// Deletes the group key (leaving the group); identity and settings stay.
    pub fn forget_group(&self) -> Result<()> {
        match std::fs::remove_file(self.root.join(GROUP)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    fn read_identity(&self) -> Result<DeviceIdentity> {
        let bytes = std::fs::read(self.root.join(IDENTITY))?;
        let secret: [u8; 32] = bytes.as_slice().try_into().map_err(|_| Error::InvalidKey)?;
        Ok(DeviceIdentity::from_secret_bytes(&secret))
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomic(path, bytes, true)
}

fn write_atomic(path: &Path, bytes: &[u8], private: bool) -> Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        if private {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        #[cfg(not(unix))]
        let _ = private;
        let mut f = opts.open(&tmp)?;
        std::io::Write::write_all(&mut f, bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> StateDir {
        StateDir::new(std::env::temp_dir().join(format!(
            "localdrop-state-{}",
            localdrop_core::JobId::random()
        )))
    }

    #[test]
    fn init_load_and_rejoin_keep_identity() {
        let d = dir();
        assert!(!d.is_joined());
        let a = d
            .init(
                "Home",
                "pw-1234567890",
                |g| Settings::new("laptop", g.name()),
                KdfParams::INSECURE_TEST,
            )
            .unwrap();
        assert!(d.is_joined());
        let loaded = d.load().unwrap();
        assert_eq!(loaded.identity.device_id(), a.identity.device_id());
        assert_eq!(loaded.key.fingerprint(), a.key.fingerprint());
        assert_eq!(loaded.settings.group_name, "home");
        assert_eq!(loaded.group.name(), "home");

        // Joining another group keeps the device identity.
        let b = d
            .init(
                "office",
                "other-password",
                |g| Settings::new("laptop", g.name()),
                KdfParams::INSECURE_TEST,
            )
            .unwrap();
        assert_eq!(b.identity.device_id(), a.identity.device_id());
        assert_ne!(b.key.fingerprint(), a.key.fingerprint());

        let mut s = b.settings;
        let key = NetworkKey([5; 32]);
        s.set_trust(key, "Home", NetworkKind::WiFi, false, NetworkTrust::Trusted);
        s.set_trust(
            key,
            "Home",
            NetworkKind::WiFi,
            false,
            NetworkTrust::NeverAsk,
        );
        d.save_settings(&s).unwrap();
        let back = d.load().unwrap().settings;
        assert_eq!(back.known_networks.len(), 1);
        assert_eq!(back.network(&key).unwrap().trust, NetworkTrust::NeverAsk);

        d.forget_group().unwrap();
        assert!(!d.is_joined());
        std::fs::remove_dir_all(d.path()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn key_files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir();
        d.init(
            "home",
            "pw",
            |g| Settings::new("x", g.name()),
            KdfParams::INSECURE_TEST,
        )
        .unwrap();
        for f in [IDENTITY, GROUP] {
            let mode = std::fs::metadata(d.path().join(f))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "{f}");
        }
        std::fs::remove_dir_all(d.path()).unwrap();
    }
}
