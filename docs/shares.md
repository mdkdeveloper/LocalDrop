# Shares, permissions and settings

Besides pushing files, a device can let other group members **browse, download,
upload to and delete in** selected folders. These folders form the device's
**whitelist**. Nothing outside the whitelist is reachable remotely.

Group membership (password) answers *"may this device talk to me?"*. The whitelist
answers *"what may it do with my files?"*. Both checks always happen on the device that
owns the files.

## Receive folders

| Setting | Default | Used for |
|---|---|---|
| `inbox_dir` | Desktop: `~/Downloads/LocalDrop`. Android: `Download/LocalDrop` (MediaStore). iOS: app's `Documents/Inbox` (visible in Files). | Files pushed to this device (drag & drop / Share) when the sender did not pick a share. |
| `deferred_inbox_dir` | *inherit* = `inbox_dir` | Files that arrive from a sender's offline queue ([deferred-send.md](deferred-send.md)). Lets the user keep "things sent while I was away" separate. |

The inbox is always writable by group members (subject to the [accept
policy](security.md#accept-policy)). It is **not** browsable remotely unless the user
also adds it to the whitelist.

- **Per-device overrides** (*Settings › Devices › device*): each known device can have its
  own `inbox_dir` and `deferred_inbox_dir`; *Like the app default* otherwise.
- **`job_subfolder`** (default **on**): a job with more than one top-level item lands in
  its own subfolder named after the job (`Inbox/Holiday 2024/…`); a single file or folder
  lands directly in the inbox. Applies to the inbox only, never to shares.
- **Picking a folder** shows the free space of its volume and warns, without blocking,
  when it is inside a [cloud sync folder](receiving.md#cloud-sync-folders) or on a FAT32
  volume (files ≥ 4 GiB can't be received there).
- Android offers `Download/LocalDrop` (MediaStore, default), *Another folder…* (SAF tree,
  persistable permission for that folder only) and the SD card when present. iOS keeps
  `Documents/Inbox`.

## Whitelist entries

```text
Share {
  id:          ShareId            // random, stable
  name:        String             // shown to peers, e.g. "Photos"
  root:        Path               // local folder
  audience:    AllMembers | Devices([DeviceId])
  permissions: SharePermissions   // each field may be Inherit
  conflicts:   ConflictPolicy     // Inherit | Ask | Auto(mode)
  backups:     BackupSettings     // Inherit | ...
}
```

- One folder may appear in several entries with different audiences and permissions
  (e.g. "Photos" read-only for all members, read-write for the user's laptop).
- If several entries match the same device and path, the **most specific audience
  wins** (an explicit `Devices` entry beats `AllMembers`); between entries of the same
  specificity, the more permissive one wins.
- Nested shares are allowed; the entry whose `root` is the deepest ancestor of the
  requested path is used.

## Permissions

| Permission | Values | Meaning |
|---|---|---|
| `browse` | `Allow` / `Deny` | List folders and see file metadata (name, size, mtime). |
| `download` | `Allow` / `Deny` | The peer may pull files from the share. Requires `browse`. |
| `upload` | `Allow` / `Deny` | The peer may push files/folders into the share. |
| `delete` | `Deny` / `Trash` / `Permanent` | Delete requests, and anything that removes or overwrites existing data (see [conflicts](#conflicts)). `Trash` moves to the OS trash (or the app trash where there is none, see [platforms.md](platforms.md)). |

### Inheritance

```text
AppSettings.permissions      (global defaults)
        ▲ inherit
Share.permissions            (each field: Inherit | explicit value)
```

- Every field of a new share is `Inherit`, so changing the app-level value changes all
  shares that did not override it.
- App defaults: `browse = Allow`, `download = Allow`, `upload = Deny`, `delete = Deny`
  (a newly added folder is read-only until the user opts in).
- The UI shows the effective value next to each field ("Trash (from app settings)").

## Remote operations

All requests carry `(share_id, relative_path)`. The owner:

1. finds the matching share for the requesting `DeviceId`;
2. resolves the path safely (below);
3. checks the effective permission;
4. performs the operation, or answers `Denied`.

| Operation | Needs | Notes |
|---|---|---|
| List shares | — | Returns only shares whose audience includes the requester, with *effective* permissions so the peer UI can grey out actions. |
| List directory | `browse` | Paginated (cursor), sorted folders-first. |
| Download files/folders | `browse` + `download` | Creates a job in the owner's outgoing queue to the requester ([transfers.md](transfers.md)). |
| Upload files/folders | `upload` | A normal job whose target is `Share{share_id, path}` instead of the inbox. |
| Delete | `delete ≠ Deny` | Applies the share's delete mode. Returns per-path results. |
| Create folder | `upload` | |

Remote operations need the owner to be **online**. Uploads into a share can also be
queued offline (the share list is cached from the last connection; permission is
re-checked on delivery).

### Path safety

- Paths on the wire are relative, `/`-separated, sanitized like file names
  ([protocol.md](protocol.md)).
- The owner joins them with `root`, canonicalizes, and rejects the result unless it is
  still inside `root`.
- Symlinks/junctions are not followed out of `root`; symlinks are listed as such but not
  traversed.
- The internal `.localdrop/` folder (backups, app trash) and in-flight `*.ldpart` temp
  files are hidden from listings and cannot be targeted.

## Conflicts

A conflict is an incoming file whose target path already exists (for a folder upload,
the owner compares the job manifest with what is on disk before any data is sent).

### Resolution modes

| Mode | Effect | Requires |
|---|---|---|
| `KeepExisting` — *upload only new files* | Conflicting files are skipped; everything else is written. | `upload` |
| `ReplaceWithBackup` | Conflicting files are overwritten; the previous versions are moved to the share's backup area first. | `upload` |
| `ReplaceConflicting` — *delete old* | Conflicting files are overwritten; old versions are removed according to `delete` (`Trash` → trash, `Permanent` → gone). | `upload` + `delete ≠ Deny` |
| `Mirror` — *replace whole folder* | Like `ReplaceConflicting`, and files in the target folder that are not in the upload are removed too (target becomes an exact copy). | `upload` + `delete ≠ Deny` |

Modes that need `delete` are not offered when `delete = Deny`.

#### Confirming `Mirror`

Choosing *Replace the whole folder* in the conflict dialog opens a second confirm
before anything is sent, because it removes files that the upload never mentions. It
shows:

- the counts: new files added, files replaced, files **removed** (the last one stands
  out);
- where removed and replaced files go, from the folder's `delete` setting (Trash on that
  device, so they can be restored);
- the first few removed files and *Show all n*;
- *Back* (to the conflict dialog) and a red button that names the number:
  **Remove 37 files and replace**.

If the folder deletes **permanently**, the button stays disabled until the user ticks
"I understand these files can't be restored". If nothing would be removed, there is no
second confirm. When a folder's conflict mode is `Auto(Mirror)` no dialog appears per
job; the same summary is shown once when that mode is set.

Identical files are never counted as conflicts and are not re-sent. "Identical" means
**same size and same mtime** (like rsync's quick check), because hashing an existing
10 GB file just to compare would take minutes. Both sides keep a hash cache
`(path, size, mtime) → content_hash` filled by earlier transfers; when both hashes are
cached they are compared too. An optional per-share "deep compare" setting hashes files
of equal size before deciding (slow for large files, off by default).

### Policy

```text
ConflictPolicy = Inherit | Ask | Auto(mode)
```

- App default: `Ask`. Every share inherits it unless overridden.
- `Auto(mode)`: the owner applies `mode` without asking anyone.
- `Ask`: **both** the sender and the owner see a dialog with the allowed modes
  (plus "Cancel"). The **first answer wins**; the other dialog closes. Neither side can
  pick a mode the owner's permissions do not allow.
- While waiting for an answer the job is paused (`AwaitingConflictDecision`) and the
  pair queue continues with the next job, so one pending dialog does not block other
  transfers. This also applies to deferred jobs: the decision is requested when they
  are delivered.

Pushes to the inbox use the same machinery with the app-level policy; for single files
the default answer offered first is "keep both" (rename to `name (1).ext`), which is
always allowed.

## Backups

`ReplaceWithBackup` moves old versions into `<root>/.localdrop/backups/` (same volume →
a rename, no copy):

```text
.localdrop/backups/<yyyy-mm-dd_hh-mm-ss>_<job_id>/<relative/path/of/file>
```

- One backup set per job, recording source device, time and file list.
- **Backup browser** in the app: list sets per share (or for all shares), with size and
  date; restore one file / whole set, delete one set, or "clear all backups"
  (whole device or one share).
- **Restoring** puts the old versions back; the current versions go to the Trash (Recycle
  Bin / app trash), so a restore can be undone too. Files deleted since then come back as
  they were. *Keep this backup set* (default on) keeps the set after restoring.
- **Clearing** backups (one set, one share or all) deletes them **permanently**, not to
  the Trash; the confirmation shows the sets and the total size. Current files are not
  touched.
- Optional retention (`BackupSettings`, inheritable like permissions): keep for N days
  and/or at most N GB per share; default *keep until cleared manually*.
- Backups are local only; peers cannot browse or delete them.

## App settings summary

```text
AppSettings {
  inbox_dir, deferred_inbox_dir
  job_subfolder: bool                   // default: true
  accept:       AcceptSettings          // auto-accept / ask, per-device overrides
  devices:      {DeviceId → DeviceSettings}  // accept, inbox_dir, deferred_inbox_dir overrides
  permissions:  SharePermissions        // defaults for shares
  conflicts:    ConflictPolicy          // default: Ask
  backups:      BackupSettings          // default: keep until cleared
  shares:       [Share]
}
```
