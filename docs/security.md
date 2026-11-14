# Security

## Threat model

**Protected against:**

- Any device on the same LAN that does not know the group password (guest Wi-Fi users,
  IoT devices, a malicious laptop in a café or office):
  - cannot tell which devices run LocalDrop *in our group* or link announcements over time;
  - cannot connect, send files, or receive files;
  - cannot read or modify traffic (passive or active MITM).
- A device that previously joined and then has its id reused by another device: key
  pinning detects the different public key.

**Not protected against (by design of the shared-password model):**

- **Anyone who knows the group password is a full member.** They can send to and receive
  from every device (subject to the accept policy). There is no per-device revocation
  short of changing the password on all remaining devices.
- **Offline password guessing from a captured announcement.** An announcement is
  encrypted under a key derived from the password, so an eavesdropper can test guesses
  offline. Argon2id makes each guess expensive, but **weak passwords can be cracked**.
  The UI must enforce a minimum strength (planned: ≥ 10 characters or a generated
  4-word passphrase).
- A compromised member device (malware with access to the app's storage).
- Traffic analysis (who talks to whom, sizes, timing) on the LAN.
- Denial of service on the LAN.

## Keys

### Device identity

- Ed25519 keypair generated on first launch; secret stored in platform secure storage
  (Keychain on iOS/macOS, Keystore-wrapped on Android, DPAPI on Windows, Secret Service
  or a `0600` file on Linux).
- `DeviceId = BLAKE3-derive-key("localdrop/v1/device-id", public_key)[..16]`.
- The TLS certificate is self-signed with this key; peers verify the certificate key,
  not a CA chain.

### Group key

```text
name_n  = trim(lowercase(name))                   (max 64 bytes)
salt    = SHA-256("localdrop/v1/group-salt" || name_n)
master  = Argon2id(password, salt, m = 64 MiB, t = 3, p = 1, len = 32)
k_disc  = HKDF-SHA256-Expand(master, "localdrop/v1/discovery", 32)
k_auth  = HKDF-SHA256-Expand(master, "localdrop/v1/auth", 32)
k_fp    = HKDF-SHA256-Expand(master, "localdrop/v1/fingerprint", 32)
fingerprint = hex(k_fp[0..2]) "-" hex(k_fp[2..4])
```

- The salt is deterministic so every device derives the same key without coordination;
  including the group name means two groups with the same password still get different keys.
- The Argon2 parameters are part of protocol v1 — changing them changes every key.
- The password is never persisted; `GroupKey` is (in secure storage) and zeroized in memory on drop.

## Discovery privacy

See [discovery.md](discovery.md). Announcements contain the `DeviceId` encrypted with
ChaCha20-Poly1305 under `k_disc` with a random nonce that rotates every 15 minutes.
Non-members see only a port and random bytes. mDNS instance names are random, never the
device name.

Discovery results are **hints**: a member could replay or forge an announcement for
another member's id, which only causes a failed connection attempt.

## Connection authentication

1. **QUIC + TLS 1.3**, both sides presenting self-signed certificates from their device
   key. This gives confidentiality, integrity and proof of possession of the device key.
2. **Membership handshake** on the first stream:
   - `ctx = TLS-Exporter("EXPORTER-localdrop-v1", 32)`;
   - symmetric **SPAKE2** with `k_auth` as the password, bound to `ctx` and both public keys;
   - key confirmation: each side sends `HMAC(spake_key, role || ctx)`.

   Without the password an attacker gets **one online guess per connection** and learns
   nothing for offline guessing; binding to the exporter prevents relaying the handshake
   between two TLS sessions. Implementations rate-limit failed handshakes per source IP.
3. **Key pinning:** after a successful handshake, the peer's public key is stored as a
   known device. Later, the same `DeviceId` with a different key is rejected
   (`Error::KeyMismatch`) and surfaced to the user.

## Authorization (shares)

Authentication proves a device is a group member; **authorization** decides what it may
do with files. Rules (details in [shares.md](shares.md)):

- Only folders in the owner's whitelist are reachable remotely; each entry names its
  audience (all members or specific `DeviceId`s, which are bound to pinned keys).
- Every request is checked **on the owner**, against the *effective* permissions
  (`browse`, `download`, `upload`, `delete = Deny | Trash | Permanent`). Permissions
  sent to peers are only hints for their UI.
- Defaults are conservative: a new share is browse + download only; upload and delete
  must be enabled explicitly (at app level or per share).
- Anything that destroys data (delete requests, `ReplaceConflicting`, `Mirror`) needs
  `delete ≠ Deny`; `ReplaceWithBackup` keeps old versions in a local backup area that
  peers can never read or delete.
- Paths are resolved inside the share root after canonicalization; symlinks are not
  followed out of the root; `.localdrop/` is never addressable.

Because all members share one password, a leaked password gives the attacker the
permissions of `AllMembers` entries. Sensitive shares should use a `Devices` audience.

## File handling on the receiver

- File names and paths from peers are sanitized (`localdrop_protocol::sanitize_file_name`)
  and resolved inside the destination root: no absolute paths, no `..`, no reserved
  Windows names or invalid characters.
- Data is written into a hidden `.<name>.ldpart` next to the target (exclusive create)
  and renamed into place only after the hash matches the sender's trailer and the data is
  flushed ([receiving.md](receiving.md)).
- Existing files are only replaced according to the [conflict
  mode](shares.md#conflicts) chosen within the owner's permissions; for pushes to the
  inbox the safe default is "keep both" (` (1)`, ` (2)`, ...).
- The receiver checks free space for the whole job (from the manifest) before accepting.
- Frames are capped at 1 MiB; manifests, paths and directory pages have size limits
  ([protocol.md](protocol.md#limits)).

## Accept policy

Default is auto-accept from group members (needed for unattended send-ahead). The user
can switch the group or individual devices to **Ask**; jobs then wait for a decision
(`JobPending`) without blocking the sender's queue.

## Changing the password

Changing the password creates a new group key; devices that don't know the new password
drop out. Known-device records are kept, but a device only becomes reachable again after
it authenticates with the new password.
