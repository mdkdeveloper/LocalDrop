# Networks

Which network interfaces LocalDrop uses for [discovery](discovery.md) and connections, how
it recognises a network it has seen before, and the per-network rules ("on at home, off at
work").

This is a convenience and privacy feature, **not** a security boundary: the group password
already hides us from everyone else ([security.md](security.md)). What it adds is that on a
network the user doesn't want, the app sends nothing at all — not even encrypted
announcements — and doesn't wait for incoming connections.

## Discovery switch

The main screen has one switch, **Discovery on / off** (desktop toolbar, phone header).

| State | Announce | Browse | QUIC listener | Pair queues |
|---|---|---|---|---|
| On | ✔ on the active networks | ✔ | ✔ | run |
| Off | — | — | closed | paused, nothing lost |

- Turning it off ends open connections after the current segment; in-flight units resume
  later ([transfers.md](transfers.md)).
- A manual change wins over the network rule until the next network change.
- *Settings › General › Visible to your group* is the finer control: browse and connect
  out, but don't announce.

## Interfaces

The node enumerates interfaces with their type, addresses, prefix and default gateway, and
classifies them:

| Class | Examples | Default |
|---|---|---|
| Wi-Fi | `wlan0`, `en0`, "Wi-Fi" | used |
| Ethernet | `eth0`, `enp3s0`, "Ethernet" | used |
| Virtual | Hyper-V / WSL `vEthernet`, Docker `docker0`/`br-*`, VirtualBox, VMware, `utun` | **ignored** |
| VPN | WireGuard, OpenVPN `tun`/`tap`, corporate VPN adapters | **ignored** |
| Cellular | `rmnet*`, `pdp_ip*` | never |
| Loopback, link-local only | `lo`, `169.254.x.x` | never |

Ignoring virtual adapters by default matters on desktop: otherwise announcements leak into
WSL/Docker networks, and a peer may be "found" on an address that can't be reached.

**Active networks** = the interfaces allowed by the user's choice (below) *and* by the rule
for the network each one is connected to.

### Choosing interfaces

- **Auto** (default): all Wi-Fi and Ethernet interfaces whose network rule allows it.
- **Only this network**: one interface/network, picked from the network menu next to the
  discovery switch. Useful when a laptop is on Wi-Fi and Ethernet at the same time, on two
  different networks.
- An advanced list in *Settings › Networks* can force-include a virtual/VPN interface (for
  example, a VPN to your home network).

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
permission), two networks with the same subnet and gateway IP are indistinguishable; the UI
says so ("Can't tell this network apart without location access").

Phone hotspots and some guest networks get a new subnet each time; they stay "unknown"
until remembered again.

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

## Network rules

```text
KnownNetwork {
  key:        NetworkKey
  label:      String          // user-editable, defaults to SSID or "Ethernet 192.168.1.x"
  kind:       WiFi | Ethernet
  rule:       Discover | DontDiscover | Ask
  last_seen:  Timestamp
}

NetworkSettings {
  selection:      Auto | Only(InterfaceRef)
  unknown_rule:   Ask | Discover | DontDiscover   // default: Ask
  include_virtual: [InterfaceRef]                 // advanced
}
```

- **Unknown network** (default rule *Ask*): discovery stays off and the app shows
  "New network: Wi-Fi *Office-5G*. Use LocalDrop here?" with **Always**, **Just this
  time**, **Never**. *Always* / *Never* remember the network; *Just this time* turns
  discovery on until the next network change.
- On desktop the question is a notification plus a banner on the main screen; on mobile,
  a sheet when the app is open (no question from the background).
- With several active networks, each follows its own rule; discovery runs only on the
  allowed ones.
- *Settings › Networks* lists remembered networks: rename, change the rule, forget.
- Pair queues to offline devices simply wait while discovery is off; nothing is dropped.

## Watching for changes

| Platform | Source |
|---|---|
| Windows | `NotifyIpInterfaceChange` / Network List Manager events |
| macOS, iOS | `NWPathMonitor` |
| Linux | netlink (`RTMGRP_LINK`, `RTMGRP_IPV4_IFADDR`) |
| Android | `ConnectivityManager.registerNetworkCallback` |

Changes are debounced (~2 s) because Wi-Fi roaming and DHCP renewals produce bursts.
