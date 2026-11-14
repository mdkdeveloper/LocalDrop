# Discovery

Discovery finds *claimed* members of our group on the LAN. It never grants trust; every
connection is authenticated by the handshake ([security.md](security.md)).

Discovery can be switched off from the main screen, and it runs only on the *active
networks*: Wi-Fi/Ethernet networks the user has marked as **trusted** (none by default;
virtual and VPN adapters only if added by hand). See [networks.md](networks.md).

## Announcement format (v1)

Implemented in `localdrop-discovery::announcement`. 51 bytes, big-endian:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | magic `LDRP` |
| 4 | 1 | version = `1` |
| 5 | 2 | QUIC listening port |
| 7 | 12 | random nonce |
| 19 | 32 | ChaCha20-Poly1305(`k_disc`, nonce, `DeviceId`, aad = bytes 0..7): 16 bytes ciphertext + 16 bytes tag |

Receiving side: parse → try to open with our `k_disc`. Failure means "not our group"
(or garbage) and the announcement is silently ignored. Success yields
`(DeviceId, port)`; the source IP of the packet/record gives the address.

### Rotation

A device generates a new nonce (and therefore completely new ciphertext) every
`ANNOUNCE_ROTATION_SECS` (15 min), on every network change, and on app start.
Non-members cannot link two announcements of the same device; the port may stay the
same, so it is randomized at startup (`listen_port = 0`).

## Backends

### mDNS / DNS-SD (primary)

- Service type `_localdrop._udp.local.`
- Instance name: random 16 hex chars, regenerated with the nonce. **Never** the device name.
- SRV/A/AAAA records as usual; port = QUIC port.
- TXT record: `v=1`, `a=<base64url(announcement)>`.
- Planned crate: `mdns-sd`. On iOS the app may need to use `NWBrowser`/`NWListener`
  through the app layer if raw multicast is restricted (see [platforms.md](platforms.md)).

### UDP broadcast (fallback)

Many routers, guest networks and some Android devices drop multicast. As a fallback,
each device:

- sends the raw 51-byte announcement every 5 s (with jitter) to the directed broadcast
  address of each active IPv4 interface (e.g. `192.168.1.255:47473`), from a socket bound
  to that interface;
- listens on UDP `47473` and treats the packet's source IP as the peer address, ignoring
  sources outside the active subnets.

### Manual address

The user can enter `ip[:port]`. The node connects directly; the handshake decides
membership.

## Peer lifecycle

- `Found` — first valid announcement, or addresses changed.
- `Lost` — no announcement for 3 × the announce interval, mDNS goodbye, or network change.
- The node merges results from all backends by `DeviceId`.
- When a `Found` peer has pending jobs in its pair queue, or the user wants to send, the node
  connects. Otherwise it may connect lazily to verify presence (shows "online").
