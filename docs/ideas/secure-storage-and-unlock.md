# Ideas: secure storage, unlock factors & decoy spaces

Status: **to be analyzed** — not scheduled. Captured 2026-11-08.

---

## 1. Storing data at rest (mobile & desktop)

Baseline (already in Phase 1):
- Everything sensitive is encrypted before it touches disk (XChaCha20-Poly1305, keys derived from
  master password + Secret Key via Argon2id + HKDF).
- The database stores **opaque records only**: a random id and an encrypted, size-padded blob.
  No plaintext titles, URLs, vault names, item counts per vault or item types.

Proposed extra layer — **device binding**:
- Wrap the local storage key a second time with a **non-exportable hardware key**:
  - iOS / macOS: Secure Enclave key (`kSecAttrTokenIDSecureEnclave`), `ThisDeviceOnly`.
  - Android: StrongBox / TEE Keystore key.
  - Windows: TPM-backed key via CNG / Windows Hello.
  - Linux: TPM2 if available, otherwise Secret Service (GNOME Keyring / KWallet).
- Effect: a copy of the database taken off the device cannot even be *attacked* (no offline
  password guessing) without the physical device.
- Sync bundles and backups use the inner (portable) keys only, so they still work across devices.
- Open questions: recovery when the hardware key is lost (OS reinstall, new phone) → recover from
  another device or an encrypted backup + Emergency Kit.

## 2. Unlock factors

| Factor | Role | Notes |
|---|---|---|
| Master password | Something you know | Always required on first unlock, after restart, every N days. |
| Secret Key (128-bit) | Something you have (the "token") | Stored on device + Emergency Kit. Already implemented. |
| Face ID / Touch ID / fingerprint / Windows Hello | Convenience unlock | Releases a hardware-wrapped copy of the unlock key. Does not replace the password cryptographically. |
| **Hardware security key (YubiKey etc.)** via FIDO2 `hmac-secret` / WebAuthn PRF | True offline 2FA | The key returns a secret that is mixed into key derivation: `AUK = HKDF(k_pw ‖ k_secretkey ‖ k_hardware)`. Works over USB/NFC on desktop and mobile. Needs a backup key enrolled. |
| TOTP codes | ❌ Not useful offline | The TOTP seed would have to be stored on the same device, so it adds no real protection without a server. |

Other ideas:
- Auto-lock on idle / sleep / screen lock; require the full password after 5 failed biometric attempts.
- Tip for users: iOS — squeezing side + volume buttons disables Face ID until passcode entry
  (useful if someone tries to force biometric unlock).

## 3. Decoy password / hidden spaces (plausible deniability)

Goal: two (or more) master passwords. One opens a believable **decoy space**, another opens the
**real space**. Under coercion the user reveals the decoy password; nobody can prove another space exists.

Sketch:
- Header holds a fixed number of **key slots** (e.g. 8). Each slot is either a real wrapped space
  key or random bytes of identical length — indistinguishable.
- One Argon2id run per unlock attempt (shared salt); the result is tried against every slot
  (AEAD tries are microseconds).
- All records from all spaces live in one table as identical-looking padded blobs; a space only
  sees the records its key decrypts. **Chaff** (random dummy records) hides how many records exist.
- A space never deletes records it cannot decrypt (they may belong to another space).
- Creating a new space requires unlocking all existing spaces in the same flow, otherwise a hidden
  slot could be overwritten.
- Sync transfers all opaque records, so it does not reveal hidden spaces either.

Known limitations to analyze:
- **Snapshot comparison**: an adversary with an older copy of the file can see records changing
  while the decoy space did not change. Mitigation: re-encrypt own records + add chaff each session;
  only partially effective.
- The decoy must look actively used (realistic items, recent edits).
- Biometric unlock must be tied to one space only — choose which.
- OS-level traces (keychain entries, autofill history, browser extension state) must not reveal the
  real space.
- Legal aspects differ per country (refusing / misleading authorities may be an offence).
- Alternative "duress password that wipes the real data" — **not recommended**: destructive,
  forensically detectable, irreversible on a mistaken entry.

Impact on current design: Phase 1 already stores opaque padded records and a versioned header,
so slots + chaff can be added later as a format v2 migration.

## 4. Emergency Kit (printable A4) & physical recovery

Yes — 1Password's **Emergency Kit** is exactly this: a printable PDF with the Secret Key, a QR code
and a blank field to hand-write the master password, meant to be kept in a physical safe.
Password365 plans the same (Phase 2, desktop setup flow); the Secret Key text format
(`Q1-XXXX-…`, with typo checksum) is already implemented in `password365-crypto`.

Contents of the Password365 Emergency Kit (A4 / Letter):
- Secret Key in large, unambiguous font (Crockford base32 — no I/L/O/U confusion) + QR code.
- Account/store identifier and creation date.
- Blank box to hand-write the master password (never printed by the app).
- Short recovery instructions ("install Password365 → Restore → scan this QR → enter password").
- Optional fingerprint (short hash) so the user can verify which kit belongs to which vault.

Ideas to analyze:
- **Shamir's Secret Sharing**: split the Secret Key into e.g. 3 shares, any 2 recover it —
  give shares to trusted people / different safes.
- **Paper backup of the whole vault**: the encrypted backup printed as a series of QR codes
  (useful for small vaults; still needs password + Secret Key to open).
- Generate the PDF fully offline and avoid OS print spoolers that keep copies where possible
  (warn the user; offer "save as PDF to USB" instead).
- Kit reprint flow requires the master password; rotating the Secret Key invalidates old kits.

## 5. Crypto-wallet style word phrases (seed phrases) — analysis & decision

Crypto wallets (BIP-39) encode random bits as 12/24 words from a fixed 2048-word list with a
built-in checksum. Words are easier than random characters to write down, read aloud and verify,
and the checksum catches typos.

Decisions:
1. **Secret Key as words — yes (Emergency Kit).** The same 128-bit Secret Key will be shown both as
   `Q1-XXXX-…` and as a **12-word phrase** (BIP-39 encoding: 128 bits + 4-bit checksum). Either form
   can be typed in. No change to the cryptography — only an alternative encoding.
2. **Optional Recovery Phrase — yes, opt-in, strongly suggested at setup.** A separate **24-word
   phrase (256 bits)** that wraps the account key independently of the master password, so a forgotten
   master password is not fatal (there is no server to reset it). Kept offline in a physical safe.
   - It is a second door into the vault, so: shown only once, never stored on any device,
     can be revoked/rotated (re-wrap) from settings, and its use is logged on all synced devices.
   - Combine with Shamir split (section 4) for people who want "2 of 3 safes".
3. **No "brain wallet" style derivation** (keys derived only from words the user chose) — users
   pick low-entropy phrases; all phrases must be generated by the CSPRNG.
4. Password generator already planned to offer **memorable passphrases** (EFF word list) for site
   passwords — same idea applied to everyday passwords.
