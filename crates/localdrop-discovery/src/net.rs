//! Network interfaces, their classification and network fingerprints
//! (see `docs/networks.md`).

use std::fmt;
use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

/// What kind of link an interface is. Only Wi-Fi and Ethernet are used by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterfaceClass {
    /// Wireless LAN.
    WiFi,
    /// Wired LAN.
    Ethernet,
    /// Hyper-V / WSL, Docker, VirtualBox, VMware and similar: only if added by hand.
    Virtual,
    /// WireGuard, OpenVPN, corporate VPNs: only if added by hand.
    Vpn,
    /// Mobile data: never.
    Cellular,
    /// Loopback: never.
    Loopback,
}

impl InterfaceClass {
    /// Whether discovery may run on it when its network is trusted.
    pub fn usable_by_default(self) -> bool {
        matches!(self, Self::WiFi | Self::Ethernet)
    }
}

/// An IPv4 address with its prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Subnet {
    /// The interface's own address.
    pub addr: Ipv4Addr,
    /// Prefix length, e.g. 24.
    pub prefix: u8,
}

impl Subnet {
    fn mask(self) -> u32 {
        if self.prefix == 0 {
            0
        } else {
            u32::MAX << (32 - u32::from(self.prefix.min(32)))
        }
    }

    /// Network address, e.g. `192.168.1.0`.
    pub fn network(self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.addr) & self.mask())
    }

    /// Directed broadcast address, e.g. `192.168.1.255`.
    pub fn broadcast(self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.addr) | !self.mask())
    }

    /// Whether `ip` is in this subnet.
    pub fn contains(self, ip: Ipv4Addr) -> bool {
        u32::from(ip) & self.mask() == u32::from(self.addr) & self.mask()
    }
}

impl fmt::Display for Subnet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network(), self.prefix)
    }
}

/// One connected network interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetInterface {
    /// OS name (`wlan0`, `en0`, a GUID on Windows).
    pub name: String,
    /// Name shown to users ("Wi-Fi", "Ethernet 2").
    pub label: String,
    /// Classification.
    pub class: InterfaceClass,
    /// IPv4 subnets (link-local ones are left out).
    pub subnets: Vec<Subnet>,
    /// Default gateway, if any.
    pub gateway_ip: Option<Ipv4Addr>,
    /// Gateway MAC address, if the OS shows it (not on Android 10+ / iOS).
    pub gateway_mac: Option<[u8; 6]>,
    /// Wi-Fi name, filled in by platform hooks when the user allowed it.
    pub ssid: Option<String>,
}

/// Lists interfaces that are up and have a usable IPv4 address.
pub fn list_interfaces() -> Vec<NetInterface> {
    netdev::get_interfaces()
        .into_iter()
        .filter(|i| i.is_up())
        .filter_map(|i| {
            let subnets: Vec<_> = i
                .ipv4
                .iter()
                .map(|n| Subnet {
                    addr: n.addr(),
                    prefix: n.prefix_len(),
                })
                .filter(|s| !s.addr.is_link_local() && !s.addr.is_unspecified())
                .collect();
            if subnets.is_empty() {
                return None;
            }
            let label = i
                .friendly_name
                .clone()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| i.name.clone());
            let class = classify(
                &i.name,
                &label,
                i.description.as_deref().unwrap_or(""),
                i.if_type,
                subnets[0].addr,
            );
            let gateway = i.gateway.as_ref();
            Some(NetInterface {
                gateway_ip: gateway.and_then(|g| g.ipv4.first().copied()),
                gateway_mac: gateway
                    .map(|g| g.mac_addr.octets())
                    .filter(|m| m.iter().any(|b| *b != 0)),
                name: i.name,
                label,
                class,
                subnets,
                ssid: None,
            })
        })
        .collect()
}

/// Classifies an interface from its type and names. Names win over the reported type,
/// because virtual and VPN adapters usually claim to be Ethernet.
pub fn classify(
    name: &str,
    label: &str,
    description: &str,
    if_type: netdev::prelude::InterfaceType,
    addr: Ipv4Addr,
) -> InterfaceClass {
    use netdev::prelude::InterfaceType as T;
    let text = format!("{name} {label} {description}").to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| text.contains(n));
    let starts = |prefixes: &[&str]| {
        let n = name.to_lowercase();
        prefixes.iter().any(|p| n.starts_with(p))
    };

    if if_type == T::Loopback || addr.is_loopback() || starts(&["lo"]) && name.len() <= 3 {
        return InterfaceClass::Loopback;
    }
    if has(&[
        "wireguard",
        "openvpn",
        "tap-windows",
        "wintun",
        "tailscale",
        "zerotier",
        "anyconnect",
        "fortinet",
        "globalprotect",
        "pangp",
        "nordlynx",
        "vpn",
    ]) || starts(&["wg", "tun", "tap", "ppp", "ipsec"])
        || matches!(if_type, T::Tunnel | T::Ppp)
    {
        return InterfaceClass::Vpn;
    }
    if has(&[
        "vethernet",
        "hyper-v",
        "virtualbox",
        "vmware",
        "vmnet",
        "docker",
        "wsl",
        "parallels",
        "loopback adapter",
    ]) || starts(&[
        "docker", "br-", "veth", "virbr", "vboxnet", "utun", "bridge", "awdl", "llw",
    ]) || if_type == T::ProprietaryVirtual
    {
        return InterfaceClass::Virtual;
    }
    if matches!(if_type, T::Wwan | T::Wwanpp | T::Wwanpp2)
        || starts(&["rmnet", "pdp_ip", "ccmni", "wwan"])
    {
        return InterfaceClass::Cellular;
    }
    if if_type == T::Wireless80211 || has(&["wi-fi", "wifi", "wireless", "wlan"]) || starts(&["wl"])
    {
        return InterfaceClass::WiFi;
    }
    InterfaceClass::Ethernet
}

/// Kind of a remembered network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkKind {
    /// Wireless.
    WiFi,
    /// Wired (and anything added by hand).
    Ethernet,
}

/// Fingerprint of a network, computed locally and stored only on this device:
/// `BLAKE3-derive-key("localdrop/v1/network", kind ‖ identifiers)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NetworkKey(pub [u8; 32]);

impl NetworkKey {
    /// Short form for the UI and CLI (first 4 bytes, hex).
    pub fn short(&self) -> String {
        self.0[..4].iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl fmt::Display for NetworkKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.short())
    }
}

impl fmt::Debug for NetworkKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NetworkKey({})", self.short())
    }
}

/// The network an interface is connected to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkId {
    /// Fingerprint.
    pub key: NetworkKey,
    /// Wi-Fi or Ethernet.
    pub kind: NetworkKind,
    /// Only subnet and gateway IP were available, so other networks with the same
    /// addresses look the same.
    pub weak: bool,
    /// Default label: the Wi-Fi name, or "Ethernet 192.168.1.x".
    pub default_label: String,
}

/// Identifies the network behind an interface with the strongest identifiers available:
/// SSID, then gateway MAC, then subnet + gateway IP.
pub fn network_id(iface: &NetInterface) -> NetworkId {
    let kind = if iface.class == InterfaceClass::WiFi {
        NetworkKind::WiFi
    } else {
        NetworkKind::Ethernet
    };
    let subnet = iface.subnets[0];
    let mut ids = vec![match kind {
        NetworkKind::WiFi => b'w',
        NetworkKind::Ethernet => b'e',
    }];
    let weak = if let Some(ssid) = &iface.ssid {
        ids.extend_from_slice(b"ssid:");
        ids.extend_from_slice(ssid.as_bytes());
        false
    } else if let Some(mac) = iface.gateway_mac {
        ids.extend_from_slice(b"gwmac:");
        ids.extend_from_slice(&mac);
        false
    } else {
        ids.extend_from_slice(b"subnet:");
        ids.extend_from_slice(&subnet.network().octets());
        ids.push(subnet.prefix);
        if let Some(gw) = iface.gateway_ip {
            ids.extend_from_slice(b"gw:");
            ids.extend_from_slice(&gw.octets());
        }
        true
    };
    let o = subnet.network().octets();
    let default_label = match (&iface.ssid, kind) {
        (Some(ssid), _) => ssid.clone(),
        (None, NetworkKind::WiFi) => format!("Wi-Fi {}.{}.{}.x", o[0], o[1], o[2]),
        (None, NetworkKind::Ethernet) => format!("Ethernet {}.{}.{}.x", o[0], o[1], o[2]),
    };
    NetworkId {
        key: NetworkKey(blake3::derive_key("localdrop/v1/network", &ids)),
        kind,
        weak,
        default_label,
    }
}

#[cfg(test)]
mod tests {
    use netdev::prelude::InterfaceType as T;

    use super::*;

    const LAN: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 20);

    #[test]
    fn subnet_math() {
        let s = Subnet {
            addr: LAN,
            prefix: 24,
        };
        assert_eq!(s.network(), Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(s.broadcast(), Ipv4Addr::new(192, 168, 1, 255));
        assert!(s.contains(Ipv4Addr::new(192, 168, 1, 7)));
        assert!(!s.contains(Ipv4Addr::new(192, 168, 2, 7)));
        let wide = Subnet {
            addr: Ipv4Addr::new(10, 1, 2, 3),
            prefix: 8,
        };
        assert_eq!(wide.broadcast(), Ipv4Addr::new(10, 255, 255, 255));
        assert_eq!(wide.to_string(), "10.0.0.0/8");
    }

    #[test]
    fn classification() {
        let c = |name, label, desc, t| classify(name, label, desc, t, LAN);
        assert_eq!(c("wlan0", "wlan0", "", T::Unknown), InterfaceClass::WiFi);
        assert_eq!(
            c("{GUID}", "Wi-Fi", "Intel(R) Wi-Fi 6E", T::Wireless80211),
            InterfaceClass::WiFi
        );
        assert_eq!(
            c("enp3s0", "enp3s0", "", T::Ethernet),
            InterfaceClass::Ethernet
        );
        assert_eq!(c("en0", "en0", "", T::Ethernet), InterfaceClass::Ethernet);
        assert_eq!(
            c(
                "{GUID}",
                "vEthernet (WSL (Hyper-V firewall))",
                "Hyper-V Virtual Ethernet Adapter",
                T::Ethernet
            ),
            InterfaceClass::Virtual
        );
        assert_eq!(
            c("docker0", "docker0", "", T::Ethernet),
            InterfaceClass::Virtual
        );
        assert_eq!(
            c("br-1a2b", "br-1a2b", "", T::Bridge),
            InterfaceClass::Virtual
        );
        assert_eq!(
            c("{GUID}", "home", "WireGuard Tunnel", T::Unknown),
            InterfaceClass::Vpn
        );
        assert_eq!(c("tun0", "tun0", "", T::Unknown), InterfaceClass::Vpn);
        assert_eq!(
            c("rmnet_data0", "rmnet_data0", "", T::Unknown),
            InterfaceClass::Cellular
        );
        assert_eq!(c("lo", "lo", "", T::Unknown), InterfaceClass::Loopback);
        assert_eq!(
            classify("x", "x", "", T::Ethernet, Ipv4Addr::LOCALHOST),
            InterfaceClass::Loopback
        );
    }

    fn iface(class: InterfaceClass, mac: Option<[u8; 6]>, ssid: Option<&str>) -> NetInterface {
        NetInterface {
            name: "if".into(),
            label: "if".into(),
            class,
            subnets: vec![Subnet {
                addr: LAN,
                prefix: 24,
            }],
            gateway_ip: Some(Ipv4Addr::new(192, 168, 1, 1)),
            gateway_mac: mac,
            ssid: ssid.map(Into::into),
        }
    }

    #[test]
    fn network_ids() {
        let weak = network_id(&iface(InterfaceClass::Ethernet, None, None));
        assert!(weak.weak);
        assert_eq!(weak.default_label, "Ethernet 192.168.1.x");

        let mac = network_id(&iface(
            InterfaceClass::Ethernet,
            Some([1, 2, 3, 4, 5, 6]),
            None,
        ));
        let other_mac = network_id(&iface(InterfaceClass::Ethernet, Some([9; 6]), None));
        assert!(!mac.weak);
        assert_ne!(mac.key, other_mac.key);
        assert_ne!(mac.key, weak.key);

        let home = network_id(&iface(InterfaceClass::WiFi, None, Some("Home")));
        assert_eq!(
            (home.kind, home.default_label.as_str(), home.weak),
            (NetworkKind::WiFi, "Home", false)
        );
        // Same SSID on another device gives the same key; it never leaves the device.
        assert_eq!(
            home.key,
            network_id(&iface(InterfaceClass::WiFi, Some([7; 6]), Some("Home"))).key
        );
        assert_eq!(home.key.short().len(), 8);
    }

    #[test]
    fn lists_this_machine() {
        // Smoke test: never panics; every listed interface has a subnet.
        for i in list_interfaces() {
            assert!(!i.subnets.is_empty(), "{i:?}");
        }
    }
}
