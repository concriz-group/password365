# Password365 sync protocol (v1 draft)

Devices sync **only with each other**: no server and no account in the cloud.
This document is the design that `core/password365-sync` implements, step by step.
Status markers: ✅ implemented · 🚧 in progress · ⏳ planned.

## 1. Replication model ✅

**Decision:** items replicate as **state-based CRDTs** (whole-item states merged
field by field), not as an operation log. PLAN.md §4.2 sketched signed change
records with version vectors. A state merge gives the same guarantees
(convergence, no lost edits) with less to store, fewer formats to get right and
nothing to replay. Signatures move to the transport/bundle layer (§4), where
they authenticate the sending device.

### 1.1 Hybrid Logical Clock (`hlc.rs`)

`Hlc { wall_ms, counter, device }`, totally ordered in that field order.

- `tick(now)` → `wall = max(last.wall, now)`, counter +1 within the same millisecond.
- `observe(remote, now)` folds in a peer's stamp, so later local edits sort after
  everything already received.
- A remote stamp more than `MAX_DRIFT_MS` (24 h) ahead of our own clock is not
  adopted. Its data still merges, but a peer with a broken clock cannot drag
  every device into the future.
- Stamps are unique per write: the same `(wall, counter)` never repeats on one
  device, and the device id breaks ties.

### 1.2 Per-field last-writer-wins (`fields.rs`, `lww.rs`)

These item fields are separate registers, each with its own stamp: kind, vault,
title, username, URLs, tags, favorite, password, notes, TOTP and archived.
Merge keeps the newer stamp per field. Editing the username on the phone and the
notes on the laptop therefore both survive.

**Custom fields and attachments (format v3, `lww_map.rs`)** are keyed maps
where *each entry* is its own register: a custom field by its label (any case),
an attachment by its id. An entry holds its value, its position and its stamp;
removing an entry writes `None` with a new stamp, so an older copy does not
bring it back while a later edit does. Moving a field is a write of its
position. The map also has its own stamp (`floor`), so an empty map can be
restored from Recently Deleted like any field. Editing one custom field on the
phone and another on the laptop keeps both. Convergence tests include adding,
removing and reordering custom fields on three devices.

**Formats.** v1: no `archived`. v2: `archived`; custom fields as one register.
v3 (current): per-entry custom fields and attachments. Each part is sealed with
its format's label (`password365/v3/item-details`, `password365/v3/item-sync`;
the head keeps `password365/v2/item-head`) and older labels are still read and
upgraded on the next write: a v2 item's custom fields all take the old single
stamp. On the wire, `Message::Items` is v3; `ItemsV2` and `ItemsV1` are read only.
A device on an older format cannot read v3 items and stops the sync without
merging anything, so **every device must be updated** before they sync again.

### 1.3 Passwords are never lost (`history.rs`, `sync_item.rs`)

- The password history merges as a **set**: one entry per password, latest
  `replaced_at` wins, newest first, capped at 32.
- Every *superseded* password write (a losing register with a different stamp)
  is added to the history, whatever the merge order. This makes the history
  identical on every device.
- If the losing value is different and was not already in either history, two
  devices set different passwords concurrently. Merge reports
  `password_conflict = true` and the UI shows "Conflict resolved — review".
- The current password is hidden from the history view.

### 1.4 Deletions (`Tombstone`)

A deletion stores `{at, seen}`, where `seen` holds the field stamps at the moment
of deletion. The item stays deleted while no field is newer than `seen`. An edit
the deleting device never saw brings the item back ("edit wins", PLAN §5.3),
even if the editing device's clock is behind.

A deleted item keeps its full encrypted state, like 1Password's **Recently
Deleted**: it can be restored, and merges stay order-independent. After 30 days
it is **purged** (`ItemState::Purged`): the values are wiped and only the
tombstone remains, so peers delete their copies too. Merging with a purged
marker deletes every copy whose writes it had seen. A copy carrying an unseen
edit comes back with that copy's values. A property test found that wiping
values *at deletion time* made merges order-dependent, which is why purging
waits. Markers are dropped for good after 180 days once every known peer has
acknowledged them ⏳.

### 1.5 Proven properties

`tests/convergence.rs` uses proptest with three devices, skewed clocks, and
random edits, deletes (recently-deleted states) and pulls. It checks that:

- merge is idempotent, commutative and associative;
- every exchange order converges to the same state;
- no password set on any device is lost.

## 2. Storage integration ✅

Implemented in `password365-vault` (`session_sync.rs`, `sync_meta.rs`). Existing
stores open unchanged: the new record kinds are appended enum variants.

- **Device record** (fixed id `ff…ff`): `{clock, seq}`. The device id is
  random, created on first unlock. **Restoring a backup issues a new one**, so
  two installs never share an id.
- **Item sync record** (id = item id with UUID version nibble `0xf`):
  `{item_id, seq, latest, body}`.
  - `body = Live{vault_id, deleted, meta}`, where `meta` is the `SyncMeta`
    (stamps, full password history, tombstone) sealed under the vault key, with
    AAD bound to vault, item and seq.
  - `body = Purged(tombstone)` means the item record is gone and only the marker
    remains.
- **Every write** goes through `Session::commit_states`, a single transaction
  that writes the item, its sync record and the device record. Each write gets a
  new **change sequence** number:
  - add or import: `capture`;
  - save: `record`;
  - delete: Recently Deleted;
  - restore: restamp every field;
  - purge;
  - merge from a peer.
- **Peers:** `changes_since(Option<seq>)` returns every state with
  `seq > since`. This includes changes received from third devices and purge
  markers. `apply_changes(states)` merges them atomically and returns
  `ApplyReport{changed, conflicts, skipped}`. Items in vaults this device does
  not have are skipped, which is the per-device vault selection.
- **Unlock:** the clock catches up on every stored stamp. Items written before
  sync existed get their first state, stamped at their `updated_at` by this
  device.
- **Recently Deleted:** `deleted_items()`, `restore_item(id)`, and
  `purge_deleted(cutoff)`. The app purges items deleted over 30 days ago on
  unlock ✅. Deleting a vault
  purges its deleted items.
- **Still to do:** `acked_seq[peer]` and peer records come with pairing (§3).
  Vault records (names, keys) replicate as part of pairing ⏳.

## 3. Pairing 🚧

Building blocks are implemented in `password365-sync::pairing` ✅:

1. **Invite** (`invite.rs`). Device A shows a QR code
   `PASSWORD365-PAIR:1:<BASE32(device_id ‖ static_x25519_pub ‖ psk ‖ addr?)>`.
   - Uppercase base32 keeps it in the QR alphanumeric mode.
   - The 256-bit `psk` is fresh for every invite and is never logged: `Debug`
     redacts it.
2. **Handshake** (`noise.rs`). Device B, the scanner, starts
   `Noise_XXpsk3_25519_ChaChaPoly_BLAKE2s` with prologue `quillock/v1/pairing`.
   - The psk authenticates the scanner. Without the QR code there is no
     handshake.
   - The scanner rejects the inviter if its static key is not the one in the
     code (`UnexpectedDevice`).
3. **Verification code.** Both screens show
   `SHA-256("quillock/v1/verification-code" ‖ handshake_hash) mod 10⁶` as
   `"ddd ddd"`, and the user confirms they match.
4. **Channel.** Transport messages are at most 65,519 bytes of plaintext. Replays
   or reordering fail, and decrypted plaintext is zeroized.
5. **Join bundle** (`password365-vault::join`) ✅. A sends
   `Session::join_bundle(secret_key, chosen_vaults)`, which contains:
   - the Secret Key;
   - the chosen vault records (names and keys);
   - A's header, used only to check the master password;
   - A as a `Peer` (device id, name, static key).

   B runs `Store::join(path, bundle, password, kdf)`. It checks the typed master
   password against A's header **before writing anything**, then creates its own
   store (own salt and storage key, same credentials) with those vaults and
   records A as a peer. B then sends `Joined{device_id, name, static_public}`
   and A pins it ✅.
7. **Over the network** (`password365-net::pairing`) ✅. Each step is one screen:
   - `PairingHost::open` listens on a free port. The QR code carries
     `ip:port`, and `local_ip()` picks the LAN interface without sending
     anything.
   - `accept` shows the code. Then `approve(vaults)` or `reject`.
   - On the new device, `Joining::connect(code)` shows the same code.
     `receive_bundle` waits for approval. A mistyped master password can be
     retried because the bundle stays in memory. Then `complete`.
   - The new device pairs with a throwaway key. Its real key is created with its
     store and sent in `Joined`. It remembers the inviter's address for the
     first sync.
   - Pairing sockets time out after 5 minutes, since people are typing and
     comparing codes.
6. **Device identity and peers** (`session_peers.rs`) ✅:
   - Each store has a long-term X25519 pairing key and a device name.
   - Peers are stored as `{device_id, name, static_public, paired_at, acked_seq,
     received_seq, last_sync}`.
   - `changes_pending_for(peer)` counts what a peer has not confirmed yet. This
     is the Devices screen's "N changes not synced".
   - Restoring a backup issues a new id **and** a new key, and forgets the peers.

**Device certificate:** `{device_id, name, platform, x25519_pub, ed25519_pub, issued_at}`,
signed with the account Ed25519 key.

**Revocation** ✅ (`session_revoked.rs`): "Remove device" adds the device to a
grow-only list (`Revoked` record, never shrinks). Every sync sends it right
after `Hello` (`Message::Revoked`), and so does every `.p365sync` file. A device
that receives it forgets those peers, so after one sync **every** paired device
refuses the removed one, not only the device where it was removed. A revoked
id can never be pinned again, and a peer that appears in the list cannot
finish a sync.

Any paired device can remove any other. There is no server and no admin:
every paired device holds the Secret Key, so they are equally trusted.

**Master password changes** ✅ (`session_password.rs`). Each store wraps its
own storage key, so a change made on one device must be applied on each of
the others. The changing device records `PasswordChange { stamp (HLC),
verifier }`, where the verifier is a header wrapping a throwaway key with the
new credentials. Every sync sends the newest one after `Revoked`
(`Message::Password`), and so do `.p365sync` files and join bundles. A device
receiving a newer change marks it pending and shows "your master password was
changed on X". The typed password is checked against the verifier, and then
the local store is re-wrapped. Until then, that device still opens with the
previous password. Neither the password nor any key travels.

**No vault-key rotation.** In this design the vault keys never travel after
pairing: items are synced as plaintext states inside the Noise channel, and
each store encrypts them again locally. Rotating keys therefore protects
nothing that removing the device does not. What the removed device already
holds cannot be taken back without a server. That copy stays encrypted with
the master password; the Secret Key is stored next to it, so the master
password is its only protection. Changing the master password elsewhere does
not re-encrypt it. If the device was lost or stolen, the remedy is changing
the passwords of the important logins themselves, and the Remove screen says so.

## 4. Transports

| Transport | Carries | Status |
|---|---|---|
| LAN TCP (`password365-net`) | 2-byte length-prefixed frames carrying Noise `XX_25519_ChaChaPoly_BLAKE2s`, prologue `quillock/v1/sync`. Both static keys are exchanged, and the remote key must be a **pinned peer key**, otherwise `UnknownDevice` | ✅ `tcp::sync_to` / `tcp::accept_sync`, 20 s timeouts |
| Fixed sync port **47480** on each desktop. Peers' addresses are remembered at pairing and refreshed on every incoming sync | | ✅ |
| LAN discovery: mDNS `_password365._tcp` (`password365-net::discovery`) | Advertised only while the sync listener runs (unlocked **and** paired). The instance name is random each time and there are no TXT records, so no device name, account or key appears. **Sync now** tries the remembered address first, then browses for 3 s; a found address is used only if the handshake shows the pinned key of that device, and the new address is then remembered | ✅ |
| `.p365sync` file (`password365-net::sync_file`) | `QLSYNC1\n` followed by frames carrying one Noise `X_25519_ChaChaPoly_BLAKE2s` message (prologue `quillock/v1/file`): **encrypted to one paired device's pinned key and authenticated by the sender's key**. Then `Hello`, `Items`…, `Done`. The import applies only a complete, authentic file from a paired sender, is idempotent, and records what the sender had (`have_from_you` → acked) | ✅ |

The pairing handshake (`XXpsk3`) and the sync handshake (`XX`) use different
patterns and prologues, so one can never be used for the other.

### 4.1 Messages and conversation ✅

These are implemented in `password365-vault::wire` and `password365-vault::peer_sync`.
They are postcard-encoded inside the Noise channel.

```
Hello { device_id, have_from_you: Option<seq> }
Items([ItemState])        chunked to the transport limit
Done  { seq }             "everything up to my seq was sent"
Ack   { seq }             "I merged your changes up to seq"
WantFiles([attachment id])          "send me these files' bytes"
FilePart { id, last, bytes }        a piece of a file's encrypted bytes
NoFile(attachment id)               "I don't have that file either"
```

`PeerSync` runs the conversation and is independent of the transport:

1. It looks up the paired device by the channel's static key. An unknown key
   gets `UnknownDevice`.
2. Both sides send `Hello`. On the peer's `Hello`, each sends
   `changes_since(have_from_you)` and then `Done`.
3. On `Done`, a side merges everything received in one transaction and records
   `received_seq`.
4. If merging created new combined states, it sends them with a new `Done`
   **before** its `Ack`, so the peer gets them in the same conversation.
5. On `Ack`, a side records `acked_seq`. The sync is finished when the peer
   acknowledged the latest `Done`, nothing is left to merge, and every file
   asked for has arrived or was answered with `NoFile`.
6. **Attachment bytes.** After merging (step 3), a side asks with `WantFiles`
   for every attachment that some item names but whose bytes it lacks; it
   sends this before its `Ack`. The peer answers each file with `FilePart`s
   or with `NoFile`. The parts are still encrypted with the attachment's own
   key, and the Noise channel encrypts them a second time. A complete file
   is kept only if an item names it and its SHA-256 matches the item's
   digest. A file that neither side has is asked for again at the next sync.
   An unrequested `FilePart`, or one larger than 50 MB plus overhead, ends
   the sync.

`.p365sync` files carry the sender's attachment bytes as `FilePart`s after
`Done`, for the items in the file.

After a sync, `changes_pending_for(peer)` is 0 on both sides. An idle sync is
at most 8 small messages.

**Protocol violations are refused:**
- messages sent before `Hello`;
- a second `Hello`;
- a `Hello` carrying a different device id than the pinned key;
- an `Ack` for something that was never sent;
- a file part nobody asked for, or a file over the size limit;
- an unreadable message;
- an item too large for a message.

## 5. Devices screen and out-of-sync alerts ⏳

- Shows each paired device with its last sync time and how many local changes
  it is missing: count the items with `seq > acked_seq[peer]`.
- A banner appears when a peer has been missing local changes for more than
  7 days, or misses 50 or more ✅ (`DeviceDto.out_of_sync`, `SyncReminder`).
- Items with a `password_conflict` get a **Review** badge ✅. The flag is kept
  per device in a local `Review` record and cleared with "Done".

## Item format v2 (2026-11-13): Archive

Items gained an `archived` flag (1Password's Archive: kept, but out of the list, search, counts, tags
and filling). Adding a field changes the binary shape, so formats are versioned:

- **At rest:** the item head and the sync stamps are sealed with new associated-data labels
  (`password365/v2/item-head`, `password365/v2/item-sync`). Opening tries v2, then v1; a v1 record
  is upgraded in memory (`archived = false`) and rewritten as v2 on the next change. The label decides
  the shape, so a record is never parsed with the wrong one. Details did not change (`quillock/v1/item-details`).
- **On the wire:** `Message::Items` (tag 1) is now `ItemsV1` (read only, upgraded on arrival);
  v2 states travel in a new appended `Message::Items`. A v1 device cannot decode it and stops the sync
  without merging anything, so both devices must run the same version to sync.
- **Merging:** an upgraded v1 state carries `archived = false` at the zero stamp, so any real
  archive/restore on another device wins.
- **Adding the next field:** list it under `added` in `password365-sync/src/fields.rs`, keep the
  previous shapes in `legacy.rs`, and bump the labels (v3).
