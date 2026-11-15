# Networks

Which network interfaces LocalDrop uses for [discovery](discovery.md) and connections, how
it recognises a network it has seen before, and which networks it trusts.

**Default: no network is trusted.** Discovery runs only on networks the user has marked as
trusted (typically "Home" during first run). On any other network the app sends nothing —
not even encrypted announcements — and doesn't accept connections. Trusting every network
is possible, but only as an explicit setting.

This is a privacy and convenience feature, **not** a security boundary: the group password
already hides us from everyone else ([security.md](security.md)).

## Discovery switch

The main screen has one switch, **Discovery on / off** (desktop toolbar, phone header).

| State | Announce | Browse | QUIC listener | Pair queues |
|---|---|---|---|---|
| On | ✔ on the active networks | ✔ | ✔ | run |
| Off | — | — | closed | paused, nothing lost |

- Turning it off ends open connections after the current segment; in-flight units resume
  later ([transfers.md](transfers.md)).
- Turning it on still only uses **trusted** networks. If none is connected, the main screen
  says so and offers to trust the current one.
- A manual change lasts until the next network change or app restart.
- **Discovery at launch** (*Settings › Networks*, default **on**): whether the switch starts
  on when the app opens. Off means the user turns it on by hand each time.
- *Settings › General › Visible to your group* is the finer control: browse and connect
  out, but don't announce.

## Interfaces

The node enumerates interfaces with their type, addresses, prefix and default gateway, and
classifies them:

| Class | Examples | Can be used |
|---|---|---|
| Wi-Fi | `wlan0`, `en0`, "Wi-Fi" | yes, if the network is trusted |
| Ethernet | `eth0`, `enp3s0`, "Ethernet" | yes, if the network is trusted |
| Virtual | Hyper-V / WSL `vEthernet`, Docker `docker0`/`br-*`, VirtualBox, VMware, `utun` | only if added by hand |
| VPN | WireGuard, OpenVPN `tun`/`tap`, corporate VPN adapters | only if added by hand |
| Cellular | `rmnet*`, `pdp_ip*` | never |
| Loopback, link-local only | `lo`, `169.254.x.x` | never |

Excluding virtual adapters matters on desktop: otherwise announcements leak into
WSL/Docker networks, and a peer may be "found" on an address that can't be reached.

**Active networks** = connected interfaces whose network is trusted (or every Wi-Fi/Ethernet
network when *Trust every network* is on), narrowed by the network menu.

### Network menu

Next to the discovery switch:

- **Trusted networks** (default): every connected network that is trusted.
- **Only *this network***: one interface/network. Useful when a laptop is on Wi-Fi and
  Ethernet at the same time, on two different networks. Only trusted networks can be picked.
- The list of connected networks with their state (*Trusted · discovering*, *Not trusted*
  with a **Trust** button, *Virtual · ignored*).

### Restricting each backend to the active networks

| Backend | How |
|---|---|
| mDNS (`mdns-sd`) | `disable_interface(IfKind::All)`, then `enable_interface(<name or address>)` for each active interface; updated on every change. |
| UDP broadcast | One socket per active interface, bound to the interface's IPv4 address; sends to that subnet's directed broadcast address (e.g. `192.168.1.255`), never to `255.255.255.255`. Received packets are accepted only if the source is in an active subnet. |
| QUIC | One endpoint bound to each active interface address (not `0.0.0.0`). Incoming connections from other subnets are refused before the handshake. Outgoing connections go to the address the peer was found on. |
| Android | Sockets are also bound to the Wi-Fi `Network` (`Network.bindSocket`) so traffic can't leave via mobile data. |
| iOS | `NWParameters.requiredInterfaceType = .wifi` / `.wiredEthernet`, or `requiredInterface` for one interface. |

A network change (interface up/down, new address, different gateway) restarts discovery on
the new set, rotates the announcement ([discovery.md](discovery.md#rotation)) and marks
peers that were only reachable through a removed interface as `Lost`.

## Recognising a network

Each connected network gets a **network fingerprint**, computed locally and stored only on
this device:

```text
NetworkKey = BLAKE3-derive("localdrop/v1/network", kind ‖ strongest identifiers available)
```

| Identifier | Strength | Needs |
|---|---|---|
| Wi-Fi SSID (+ BSSID list seen) | strong | location permission on most platforms (below) |
| Gateway MAC address (from the ARP/neighbour table) | strong | nothing on desktop; **not available** on Android 10+ and iOS |
| Windows network profile id (Network List Manager) | strong | nothing (to verify on 24H2+) |
| Subnet + gateway IP (e.g. `192.168.1.0/24` via `.1`) | weak: many home routers share it | nothing |
| DHCP domain / DNS suffix | weak, helps at work | nothing |

The strongest available set is used. A network seen once with weak ids and later with
strong ids is upgraded in place. When only weak ids exist (a phone without location
permission), two networks with the same subnet and gateway IP are indistinguishable, so
trusting one trusts the other; the UI says so when it trusts a network by weak ids only
("Other networks with the same address will be trusted too").

Phone hotspots and some guest networks get a new subnet each time; they stay "not trusted"
until trusted again.

### Getting the Wi-Fi name

| Platform | API | Requirement |
|---|---|---|
| Windows | Native Wi-Fi (`WlanQueryInterface`) | Windows 11 24H2+: Location must be on for desktop apps, or the API returns no SSID |
| macOS | CoreWLAN `CWInterface.ssid()` | macOS 14+: Location Services authorization; no entitlement bypasses it |
| Linux | NetworkManager over D-Bus | none |
| Android | `ConnectivityManager` network callback with `FLAG_INCLUDE_LOCATION_INFO` | `ACCESS_FINE_LOCATION` granted and location turned on |
| iOS | `NEHotspotNetwork.fetchCurrent` | "Access Wi-Fi Information" entitlement + precise location authorization |

The app asks for location **only** when the user wants Wi-Fi names, with the reason
spelled out; everything else works without it. The SSID never leaves the device.

## Trusted networks

```text
KnownNetwork {
  key:        NetworkKey
  label:      String          // user-editable, defaults to SSID or "Ethernet 192.168.1.x"
  kind:       WiFi | Ethernet
  trust:      Trusted | NotTrusted | NeverAsk
  last_seen:  Timestamp
}

NetworkSettings {
  trust_every_network: bool                 // default: false
  selection:           TrustedNetworks | Only(InterfaceRef)
  discover_at_launch:  bool                 // default: true
  on_new_network:      AskToTrust | StaySilent   // default: AskToTrust
  include_virtual:     [InterfaceRef]       // advanced
}
```

- **First run.** The join form has *Trust this network (Wi-Fi “Home”)*, checked by
  default: the network the device is on while joining is the natural "home". Unchecking it
  joins the group without trusting any network yet.
- **A network that isn't trusted** (new or *NotTrusted*): discovery stays off on it. With
  *Ask to trust*, the app asks once per new network: "LocalDrop only searches on networks
  you trust. Trust “Office-5G”?" — **Trust this network**, **Not now**, **Never ask**.
  *Not now* keeps it *NotTrusted* and asks again next time; *Never ask* stops the question
  for that network. With *Stay silent*, nothing is asked; the user trusts networks from
  the network menu or settings.
- On desktop the question is a notification plus a banner on the main screen; on mobile, a
  sheet when the app is open (never from the background).
- **Trust every network** (*Settings › Networks*, default off, marked "not recommended"):
  every Wi-Fi/Ethernet network counts as trusted; virtual and VPN adapters still need to be
  added by hand.
- With several connected networks, discovery runs only on the trusted ones.
- *Settings › Networks* lists remembered networks: rename, change trust, forget.
- Pair queues to offline devices simply wait while no trusted network is connected; nothing
  is dropped.

## Watching for changes

| Platform | Source |
|---|---|
| Windows | `NotifyIpInterfaceChange` / Network List Manager events |
| macOS, iOS | `NWPathMonitor` |
| Linux | netlink (`RTMGRP_LINK`, `RTMGRP_IPV4_IFADDR`) |
| Android | `ConnectivityManager.registerNetworkCallback` |

Changes are debounced (~2 s) because Wi-Fi roaming and DHCP renewals produce bursts.

**Current implementation:** the node polls the interface list every 2 s (`netdev`) and
acts on a change once it has stayed the same for one more poll. This works the same on
every platform; the OS notifications above replace polling later. On a change the
discovery switch returns to *Discovery at launch*, the announcement rotates, and peers
only reachable through a removed interface are reported lost.

**Network keys today:** gateway MAC address when the OS shows it, otherwise subnet +
gateway IP (marked weak). The Windows network profile id and Wi-Fi names (SSID, through
platform hooks) are not used yet.
