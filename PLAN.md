# Pass — Offline-First Password Manager: Implementation Plan

> **Historical planning document.** The product is now called Password365 and some details below
> changed along the way; the current behavior is in the README, `docs/` and the code.

> Working name: **Pass**. A 1Password-style password manager for iOS, Android, macOS, Windows
> (and Linux for free), with a browser extension. **No cloud, no server, no account backend.**
> Devices sync only with each other, directly: local Wi-Fi, Bluetooth/proximity, or an
> encrypted file you move yourself.

---

## 1. Guiding principles

1. **Zero servers.** No telemetry, no account service, no remote config. Every network call is opt-in and
   listed in the UI (e.g. optional breach check, favicon fetch).
2. **One audited core.** All cryptography, vault storage, merge logic, import/export and the password
   generator live in **one Rust library**. The apps are thin UIs on top of it. Crypto is written once,
   tested once, audited once.
3. **Standard, well-vetted primitives — not "stacked" exotic ones.** Strength comes from a modern
   memory-hard KDF, a second secret factor, authenticated encryption and a sound key hierarchy. Chaining
   multiple ciphers (AES→Twofish→Serpent) adds complexity and attack surface, not security. We *do* add
   post-quantum hybrid key exchange where it buys real protection (data recorded today, decrypted later).
4. **The UI should match 1Password's daily flows** (unlock → search → fill; inline field menus; save/update
   prompts; Cmd/Ctrl+\ fill shortcut; Watchtower-style health) — with our own branding and visual design.
5. **Losing every device must not mean losing everything** — with no server, backups and the Emergency
   Kit are first-class features, not afterthoughts.

---

## 2. Technology choices

| Layer | Choice | Why |
|---|---|---|
| **Core library** | **Rust** | Memory safety, no GC (secrets can be zeroized), excellent crypto crates, compiles to every target. 1Password and Bitwarden both moved their cores to Rust for this reason. |
| Core → Swift/Kotlin bindings | **UniFFI** (Mozilla) | Generates idiomatic Swift and Kotlin bindings from one interface definition. |
| Core → desktop | Direct Rust (Tauri backend) | No FFI boundary at all on desktop. |
| **Desktop app (Windows, macOS, Linux)** | **Tauri 2** + **React + TypeScript** | Rust backend = the core runs natively; small binaries; strict IPC allow-list; the web UI is shared with the extension popup. |
| **iOS app** | **Swift + SwiftUI** | AutoFill Credential Provider extension, passkeys, Secure Enclave, Face ID and Multipeer Connectivity all require native APIs. |
| **Android app** | **Kotlin + Jetpack Compose** | Autofill Service, Credential Manager (passkeys), StrongBox Keystore, BiometricPrompt are native APIs. |
| **Browser extension** | **TypeScript**, Manifest V3, built with **WXT** | One codebase for Chrome, Edge, Brave, Firefox, and Safari (Safari wrapped in the macOS app). |
| Extension ↔ app | **Native Messaging** to the desktop app | The extension never holds the vault; it asks the app for matches per origin (same model as 1Password). |
| Local storage | **SQLite** (via `rusqlite`) with every sensitive column encrypted by the core | Robust, transactional, everywhere. |
| Shared UI | `packages/ui` (React components) | Used by desktop app and extension popup/inline menu. |

Why not Flutter / React Native for everything? Mobile autofill, passkeys, biometric key storage and
proximity transports are all native-only APIs — you'd write most of the hard parts natively anyway.
Native mobile + shared Rust core + shared TS UI for desktop/extension gives **3 UI stacks, 1 security core**.

### Monorepo layout

```
pass/
├── core/                      # Rust workspace
│   ├── pass-crypto/           # KDF, AEAD, keys, key hierarchy, zeroization
│   ├── pass-vault/            # item model, SQLite store, search index, vaults
│   ├── pass-sync/             # change log, HLC/vector clocks, merge, Noise sessions, bundles
│   ├── pass-transport/        # LAN (mDNS + TCP/QUIC), BLE framing, file bundles
│   ├── pass-generator/        # passwords, passphrases, PINs, site rules
│   ├── pass-import/           # Chrome/Edge/Firefox/Safari CSV, Bitwarden JSON, 1PUX, CXF
│   ├── pass-health/           # weak/reused/old/compromised (Watchtower-like)
│   ├── pass-ffi/              # UniFFI bindings for Swift/Kotlin
│   └── pass-cli/              # dev/test CLI (also useful for power users)
├── apps/
│   ├── desktop/               # Tauri 2 (Rust) + React/TS frontend
│   ├── apple/                 # Xcode: iOS app, AutoFill ext, (later) macOS AutoFill ext, Safari ext host
│   └── android/               # Gradle: app, autofill service, credential provider
├── extension/                 # WXT MV3 extension (content scripts, background, popup)
├── packages/ui/               # shared React components & design tokens
└── docs/
    ├── threat-model.md
    ├── crypto-spec.md         # exact formats, test vectors
    └── sync-protocol.md
```

---

## 3. Security architecture

### 3.1 Two-secret key derivation (master password + Secret Key)

Even without a server, we adopt 1Password's "two-secret" model: a random **Secret Key** (128 bits,
shown as `A3-XXXXXX-XXXXXX-…`) generated on first setup and stored only on enrolled devices and in the
printed Emergency Kit. A stolen vault file or sync bundle is then uncrackable even if the master password
is weak, because the attacker also needs 128 bits of random key.

```
salt            = 32 random bytes (per account, stored in vault header)
pw_norm         = NFKD(trim(master_password))
k_pw            = Argon2id(pw_norm, salt, m=256 MiB, t=3, p=4, out=32)   # desktop
                  Argon2id(..., m=128 MiB, t=4, p=4)                      # mobile (benchmarked at setup)
k_sk            = HKDF-SHA256(ikm=secret_key, salt=account_id, info="pass/v1/sk", 32)
AUK             = HKDF-SHA256(ikm=k_pw || k_sk, salt=salt, info="pass/v1/auk", 32)   # Account Unlock Key
```

- KDF parameters are **stored in the header and versioned**, so they can be raised later without
  breaking old vaults (re-wrap on next unlock).
- Setup screen benchmarks the device and targets ~0.5–1 s unlock on the slowest enrolled device.
- Master password strength enforced with **zxcvbn** (min score 3) plus a check against a bundled
  offline list of common passwords.

### 3.2 Key hierarchy (implemented in Phase 1 — see `docs/crypto-spec.md`)

```
AUK (derived, never stored)
 └─ wraps → storage key (random 256-bit)            header (AAD binds salt + KDF params)
              └─ seals → records (opaque, padded)   one per vault and per item
                   ├─ Vault record: name + Vault Key (random 256-bit; Personal, Work, …)
                   └─ Item record: head + details, each sealed under its Vault Key
```

- Planned for Phase 4 (sync): account keyset with X25519 + ML-KEM-768 and Ed25519 keypairs.
- Per-item keys were dropped: moving an item re-encrypts it under the target vault key (cheap).
- **Every encrypted blob is AEAD with associated data** binding it to its context
  (store, record, vault, item, version), so ciphertexts cannot be swapped or mixed.
- **Head vs. details:** title, URLs, username and tags ("head") are encrypted separately from
  password/notes/TOTP ("details"), so listing and search never decrypt secrets.

### 3.3 Primitives (all from audited libraries)

| Purpose | Primitive | Crate |
|---|---|---|
| Password KDF | **Argon2id** | `argon2` (RustCrypto) |
| Key derivation | **HKDF-SHA-256** | `hkdf` |
| Symmetric AEAD | **XChaCha20-Poly1305** (192-bit random nonces — no nonce-reuse risk) | `chacha20poly1305` |
| Key agreement | **X25519 + ML-KEM-768 hybrid** (post-quantum) | `x25519-dalek`, `ml-kem` (or `aws-lc-rs`) |
| Signatures | **Ed25519** (ML-DSA hybrid as a later upgrade) | `ed25519-dalek` |
| Secure channel | **Noise Protocol** `XXpsk3` with hybrid PQ handshake | `snow` + custom PQ KEM step |
| RNG | OS CSPRNG | `getrandom` / `rand_core::OsRng` |
| Secret handling | `zeroize`, `secrecy`, `subtle` (constant-time compare) | |

### 3.4 Unlock & device-local protection

- **Biometric / quick unlock:** after a full password unlock, the AUK is wrapped by a hardware-bound key:
  - iOS/macOS: **Secure Enclave** key, Keychain item with `.biometryCurrentSet` + `ThisDeviceOnly`.
  - Android: **StrongBox Keystore** key, `setUserAuthenticationRequired(true)`, invalidated on new
    biometric enrollment.
  - Windows: **Windows Hello** + TPM-backed key (`KeyCredentialManager`), DPAPI-NG fallback.
- Full master password required: on restart, every N days (default 14), after 5 failed biometric tries.
- **Auto-lock:** on sleep/screen lock, idle timeout (default 10 min desktop / 1 min mobile), app switch
  (optional).
- **Memory hygiene:** secrets in `Secret<…>` types, zeroized on drop, `mlock` where available, never
  logged (custom `Debug` impls redact), no secrets crossing into JS on desktop except the single value
  being displayed/copied.
- **Clipboard:** auto-clear after 90 s; marked sensitive so it stays out of clipboard history
  (macOS `org.nspasteboard.ConcealedType`, Windows `ExcludeClipboardContentFromMonitorProcessing`,
  Android `ClipDescription.EXTRA_IS_SENSITIVE`).
- **Screens:** Android `FLAG_SECURE`, iOS blur on app-switcher snapshot, desktop content protection.

### 3.5 Engineering security practices

- `docs/threat-model.md` written **before** code (STRIDE over: stolen device, stolen backup/bundle,
  malicious website, malicious extension, malware on LAN, malicious paired device, shoulder surfing).
- `docs/crypto-spec.md` with exact byte formats + **published test vectors**; every platform test-suite
  runs them.
- `cargo-fuzz` on every parser (vault file, sync messages, CSV/JSON imports, bundle format).
- Property tests (`proptest`) for the merge engine (convergence, idempotence, commutativity).
- `cargo-deny` + `cargo-vet` for dependencies; minimal dependency count; lockfiles pinned.
- Extension: strict CSP, no remote code, no `eval`, minimal permissions.
- Reproducible builds, signed releases, signed updates (Tauri updater with Ed25519; app stores on mobile).
- **External security audit + pentest before 1.0.**

---

## 4. Data model

### 4.1 Account, vaults, items

- **Account** = one person's identity (master password + Secret Key). One account per install;
  multiple accounts (e.g. a separate employer-managed one) supported later.
- **Vaults:** any number — *Personal*, *Work*, *Family*, … each with its own Vault Key, icon, color.
  - **Per-device vault selection:** choose which vaults exist on which device (e.g. *Work* not on the
    personal phone). Sync never sends a vault key to a device not authorized for it.
- **Item types:** Login, Password, Secure Note, Credit Card, Identity, Bank Account, SSH Key,
  API Credential, Software License, Document (attachment), Wi-Fi network, **Passkey**.
- **Login fields:** title, username, password, URLs (multiple, with match mode: domain / host / exact /
  never), **TOTP** (built-in authenticator), notes, tags, custom fields, attachments, favorite,
  **password history** (previous values kept, encrypted).

### 4.2 Change tracking (needed for serverless sync)

- Every mutation produces a **signed change record**:
  `{item_uuid, vault_uuid, field_patches, HLC timestamp, device_id, prev_version_hash, signature}`.
- **Hybrid Logical Clock** per device + **version vector** per vault (`{device_id: counter}`).
- Deletes are **tombstones** (kept 180 days, then compacted once all known devices have acked).

---

## 5. Offline peer-to-peer sync

### 5.1 Device pairing (one-time, in person)

1. Existing device shows a **QR code** containing: device ID, its X25519+ML-KEM public keys, a one-time
   256-bit pairing PSK, and the LAN address (if any).
2. New device scans it (or types a 6-word code if no camera) and runs a Noise `XXpsk3` handshake.
3. Both screens show a **short verification code** (SAS, 6 digits/emoji) — user confirms they match.
4. Existing device sends the Account Keyset (wrapped via the hybrid KEM) and the Vault Keys for the
   vaults the user selected for this device; then issues a **device certificate** signed with the
   account Ed25519 key.
5. New device asks for the master password (it still needs the Secret Key, which was transferred in the
   encrypted channel — or typed from the Emergency Kit).

**Device revocation:** a signed revocation record propagates through sync; revoked devices are refused,
and the user is offered "rotate vault keys" (re-encrypt everything under new keys).

### 5.2 Transports (all carry the same encrypted Noise session)

| Transport | Platforms | Use |
|---|---|---|
| **LAN (Wi-Fi)**: mDNS/Bonjour discovery + TCP (QUIC later) | all | Default, fastest. Discovery advertises only a rotating opaque ID, never the name/account. |
| **Apple Multipeer Connectivity** | iOS ↔ macOS | Works without a shared Wi-Fi network (peer-to-peer Wi-Fi + BT). |
| **Bluetooth LE GATT** | all (Windows via WinRT BLE) | Cross-platform fallback when no shared network; deltas only (slow). |
| **Encrypted sync bundle file** (`.passsync`) | all | Move via USB stick, AirDrop, Nearby/Quick Share, email to self, etc. Encrypted to the recipient device's public keys + signed by sender — useless to anyone else. |
| Wi-Fi Direct / Wi-Fi Aware | Android ↔ Android | Later optimization. |

### 5.3 Sync protocol

```
A → B : Hello {device_cert, vault list, version vectors}
B → A : Hello {…}
both  : compute missing change-sets per shared vault (vector diff)
A ⇄ B : send signed change-sets (chunked, resumable)
both  : verify signatures + device certs → merge → persist → Ack {new vectors}
```

**Merge rules (deterministic, proven by property tests):**
- Per-**field** last-writer-wins by HLC (so editing the username on phone and notes on laptop both survive).
- If the **same secret field** (password, TOTP secret) changed on both sides since the common ancestor:
  keep the newer one as current, push the other into **password history**, and raise a visible
  "conflict resolved — review" badge. Never silently lose a password.
- Delete vs. edit → edit wins, item restored, user notified.

### 5.4 "Out of sync" awareness

- Each device stores the last version vector it saw from every peer → it knows exactly how many of its
  changes each peer is missing.
- UI: a **Devices** screen — "MacBook · last synced 3 days ago · 7 changes not on this device's peers".
- Badge/notification when: a peer is behind by > N changes or > X days (configurable), or a known peer is
  **seen on the LAN with a different state hash** → one-tap "Sync now".
- Optional **auto-sync** when trusted peers are on the same network (desktop always; mobile when app is
  foregrounded, since iOS/Android restrict background networking).
- Item editor warns when you edit an item that a peer changed more recently than your last sync with it.

### 5.5 Backups & recovery (critical with no server)

- **Emergency Kit PDF** at setup: Secret Key, QR code, blank line for the master password.
- **Encrypted backup file** (`.passbackup`): full export encrypted with the AUK-derived key, scheduled
  reminders ("No backup in 30 days").
- Recovery path = Emergency Kit + master password + any backup or any paired device.

---

## 6. Feature specifications

### 6.1 Password generator
- **Random:** length 8–100, char classes toggles, exclude ambiguous chars; uniform via
  **rejection sampling** (no modulo bias); guarantees ≥1 char of each enabled class without biasing.
- **Memorable passphrase:** EFF large wordlist (7,776 words), separator, capitalization, digit option.
- **PIN:** 4–12 digits.
- Live **entropy** (bits) + strength meter.
- **Site-aware rules:** honor the HTML `passwordrules` attribute, `maxlength`/`pattern` on the input, and
  a bundled quirks database (sites known to reject certain symbols / lengths).
- Generated passwords are saved immediately to a **"Generated password" history** so a crashed signup
  never loses the password.

### 6.2 Browser extension (1Password-like UX)
- **Inline field icon** in username/password fields → dropdown with matching logins (rendered in an
  extension-owned iframe/closed shadow root, not page DOM).
- **Fill shortcut** `Cmd/Ctrl+\`, popup with search (`Cmd/Ctrl+Shift+X` style), copy username/password/TOTP.
- **Auto-fill TOTP** into one-time-code fields after login fill.
- **Save prompt on login:** detect form submission (submit event, button click + password field present,
  `fetch`/XHR with credentials followed by navigation) → if credentials not in vault → "Save to *Personal*?"
  with vault picker.
- **Update prompt:** submitted password differs from stored one → "Update password for x@y.com?".
- **Sign-up detection:** heuristics on `autocomplete="new-password"`, two password fields, "confirm",
  "create account/register" text, field classifier → offer **Suggest strong password**, fill both fields,
  then prompt to save after submit.
- **Field classifier:** `autocomplete` attributes → labels/names/ids/placeholder heuristics (multi-language)
  → later a small on-device ML model. Handles multi-step logins (username page, then password page).
- **Phishing & attack protection:**
  - Match on **eTLD+1 via the Public Suffix List**; never fill on lookalike domains; HTTPS-only by default.
  - **Never auto-fill on page load** — fill requires a user gesture (defeats hidden-form harvesting).
  - Don't fill into cross-origin iframes unless same site and visible.
  - Defend against **DOM-based clickjacking of the inline menu** (publicly demonstrated against major
    password managers in 2025): verify the menu's iframe is topmost/visible/opaque at click time,
    detect overlays and `opacity`/`pointer-events` tricks, require a confirmation popup on suspicious pages.
- **Native-messaging trust:** the desktop app verifies the extension ID and the browser's code signature;
  the extension verifies the app's identity; then they establish an encrypted session (key confirmation shown
  once in both UIs). The extension unlocks/locks with the app.
- **Safari:** Safari Web Extension bundled inside the macOS app (same TS code).

### 6.3 Mobile autofill
- **iOS:** AutoFill Credential Provider extension (`ASCredentialProviderViewController`) for passwords,
  **passkeys** and one-time codes; QuickType bar suggestions; app ↔ website association matching.
  Note: iOS restricts third-party "save password" prompts — check the current iOS SDK at implementation time
  for what's allowed; otherwise offer a share-sheet/manual save flow.
- **Android:** `AutofillService` (inline keyboard suggestions on Android 11+, **save requests** via
  `onSaveRequest` for "save this login?"), plus **Credential Manager provider** (Android 14+) for passwords and
  passkeys. Map apps to websites via Digital Asset Links.

### 6.4 Watchtower-style health (offline)
- Weak (zxcvbn), **reused**, old (> 1 year), missing 2FA (bundled 2fa directory list), HTTP-only URLs,
  expiring cards/documents.
- **Compromised passwords:** opt-in, using HIBP k-anonymity (only a 5-char SHA-1 prefix leaves the device),
  *or* fully offline using a downloadable compressed breach filter (binary fuse filter) the user imports
  manually — preserves the "no network" promise.

### 6.5 Import / export
- **Chrome CSV import** (same format for Edge/Brave/Opera): columns `name,url,username,password,note`.
  - Preview screen, choose target vault, **deduplicate** (same URL+username → merge/skip/keep both),
    report failures.
  - After import: warn that the CSV is **plaintext** and offer to securely delete it.
- **Chrome CSV export** (to import into Chrome via `chrome://password-manager/settings`): explicit
  warning + master password re-prompt, optional per-vault selection.
- Also: Firefox CSV, Safari CSV, Bitwarden JSON, 1Password `.1pux`, and the **FIDO Credential Exchange
  Format (CXF)** — the emerging standard for moving passwords *and passkeys* between managers.
- Native **encrypted export** (`.passbackup`) for moving between Pass installs.

---

## 7. Phased roadmap

| Phase | Scope | Exit criteria |
|---|---|---|
| **0. Specs** (2–3 wks) | Threat model, crypto spec + test vectors, sync protocol spec, UX wireframes | Specs reviewed; formats frozen as v1 |
| **1. Core** (5–7 wks) | `pass-crypto`, `pass-vault`, `pass-generator`, `pass-import` (Chrome CSV), `pass-cli` | CLI can create account, vaults, items, generate, import/export Chrome CSV; fuzzers & test vectors green |
| **2. Desktop** (5–6 wks) | Tauri app: setup, Emergency Kit, unlock (password + Touch ID/Windows Hello), vaults, item editor, search, generator, clipboard, auto-lock, health | Daily-usable on macOS + Windows |
| **3. Extension** (6–8 wks) | Native messaging, inline menu, fill, save/update prompts, signup + generator, TOTP, phishing protections; Chrome/Edge/Firefox | Fills & saves on top 100 sites test corpus |
| **4. Sync** (6–8 wks) | Change log, merge engine, pairing via QR, LAN transport, `.passsync` bundles, devices screen, out-of-sync alerts, revocation | Two desktops converge under randomized concurrent-edit tests |
| **5. iOS** (6–8 wks) | SwiftUI app, Face ID, AutoFill extension, Multipeer + LAN sync, QR pairing | Phone ⇄ Mac sync and Safari/app autofill working |
| **6. Android** (6–8 wks) | Compose app, biometrics, Autofill service + save, Credential Manager, LAN/BLE sync | Phone ⇄ desktop sync and autofill working |
| **7. Advanced** | Passkeys everywhere, BLE transport on all platforms, Safari extension, CXF, 1PUX/Bitwarden import, offline breach filter, attachments | Feature parity target reached |
| **8. Hardening & release** | External audit, pentest, bug bounty, reproducible builds, store submissions | Audit findings closed → 1.0 |

Building desktop + extension before mobile gives a usable product early and exercises the core + sync
between two desktops before adding mobile platform constraints.

---

## 8. Open decisions to confirm

1. **Product name / branding** (avoid anything resembling 1Password's marks — same flows, own look).
2. **Optional network features:** allow opt-in HIBP breach checks and favicon fetching, or keep the apps
   strictly air-gapped (offline breach filter only)?
3. **Linux desktop:** Tauri gives it almost for free — ship it?
4. **Open source?** Strongly recommended for a security product (trust + community review); pick license
   (e.g. AGPL-3.0 or GPL-3.0 for apps, Apache-2.0/MIT for the core crypto spec/test vectors).

## 9. First concrete steps

1. `git init`, create the monorepo skeleton above, Rust workspace with CI (fmt, clippy `-D warnings`,
   tests, `cargo-deny`).
2. Write `docs/threat-model.md` and `docs/crypto-spec.md`.
3. Implement `pass-crypto` (KDF → AUK → keyset → vault/item keys, AEAD with AAD) + test vectors + fuzz targets.
4. Implement `pass-vault` SQLite schema and `pass-cli` (`init`, `unlock`, `add`, `get`, `gen`, `import chrome`).
