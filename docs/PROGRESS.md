# Progress

Living status of the implementation. See `PLAN.md` for the full plan,
`docs/QUESTIONS.md` for open decisions and `docs/ideas/` for ideas to analyze later.

## MVP definition

The MVP is reached when a user can, on desktop (macOS / Windows / Linux):
1. Create an account (master password + Secret Key + Emergency Kit) and unlock it.
2. Manage multiple vaults and Login / Password / Secure Note items.
3. Generate passwords / passphrases / PINs.
4. Import from and export to Chrome CSV.
5. Use it from the terminal (CLI) — **Phase 1 MVP ✅** — then from the desktop app and the browser
   extension (fill + save prompt) — **product MVP**.

## Phase 1 — Rust core + CLI ✅ (2026-11-08)

| Module | Status | Tests | Line coverage |
|---|---|---|---|
| `password365-crypto` | ✅ | 84 | ~99.7 % |
| `password365-model` | ✅ | 40 | 100 % |
| `password365-generator` | ✅ | 30 | ~99.6 % |
| `password365-vault` | ✅ | 65 | ~99.4 % |
| `password365-import` | ✅ | 26 | 100 % |
| `password365-cli` (+3 e2e tests on the real binary) | ✅ | 53 | ~97 % |
| **Workspace** | ✅ | **298** | **99.25 %** |

(Core after Phase 2 work: password365-app 52 tests; desktop frontend 81 tests, 99.4 % lines.)

Also done:
- `docs/crypto-spec.md`, `docs/threat-model.md`, `README.md`, `LICENSE` (GPL-3.0-or-later).
- CI (`.github/workflows/ci.yml`): fmt, clippy `-D warnings`, tests on Linux/macOS/Windows,
  coverage gate ≥ 90 %, `cargo-deny` (advisories, licenses, sources).
- Fixed RUSTSEC-2026-0385 (`ctutils` < 0.4.4, pulled in by argon2 → blake2 → digest) by updating
  to 0.4.8; MSRV raised to Rust 1.87.
- Manual end-to-end run with production Argon2id params: init ≈ 1 s, unlock ≈ <1 s, no plaintext
  in the store file, files created 0600.

### Phase 1 follow-ups (small, not blocking)
- [x] Publish JSON test vectors (password + Secret Key → AUK) for other-language ports — cross-checked with OpenSSL's Argon2id and Python's HKDF.
- [ ] Windows: restrict ACLs on store / Secret Key / export files.
- [ ] Master password strength meter (zxcvbn) instead of the 10-char minimum only.
- [x] Robustness: property tests (stable Rust, instead of cargo-fuzz) feed thousands of random inputs to every reader — CSV, 1PUX, Bitwarden JSON, KDBX, Secret Key, TOTP, SSH agent messages and keys, vault files. They found and fixed two crashes on multi-byte text (TOTP parsing, card expiry).
- [ ] Clipboard copy with auto-clear (`password365 item copy`).
- [ ] Emergency Kit PDF generation (planned in the desktop app).

## Phase 2 — Desktop app (Tauri 2 + React/TS) — in progress

| Step | Status | Notes |
|---|---|---|
| `password365-app` service layer | ✅ 44 tests, ~99.5 % | UI-agnostic `AppService`: account, unlock/lock, auto-lock, vaults, items, reveal-on-demand, generator, CSV import/export; DTOs never carry secrets; stable error codes |
| CLI refactored onto shared code | ✅ | password policy, Secret Key store, import/export now come from `password365-app` (duplicates removed) |
| Tauri scaffold + IPC commands | ✅ 4 Rust tests | `apps/desktop/src-tauri` (own Cargo workspace): 18 one-line commands over `AppService`, idle auto-lock thread emitting `vault-locked`, strict CSP, `freezePrototype`, only `core:default` capability, store and Secret Key in separate OS dirs |
| Frontend foundation | ✅ 24 tests, 99 % lines | React 19 + TS 7 + Vite 8; typed API client with injectable `invoke`; onboarding (Secret Key shown + "I saved it" gate), unlock (asks Secret Key only on a new device), auto-lock routing |
| Main three-pane UI | ✅ 66 tests, 99.6 % lines | sidebar (vaults + create), list + search, detail (secrets fetched only on reveal/copy, clipboard auto-clear 90 s), create/edit form (notes fetched only when editing), delete with inline confirm, generator (password/passphrase/PIN) |
| Browser demo mode | ✅ | `npm run dev` + open http://localhost:1420 → in-memory fake backend (password `demo`); dev-only, verified absent from production bundle. Used to visually review the UI |
| Import/export UI | ✅ | native open/save dialogs (`tauri-plugin-dialog`, only `allow-open`/`allow-save`), vault choice, "update existing" option, summary with skipped lines, plaintext warnings + mandatory acknowledgement before export |
| Settings | ✅ | auto-lock 1 min – 4 h (no "never"), persisted in `settings.json` (validated, atomic write, falls back to defaults); change master password |
| Secret Key in OS keychain | ✅ | `keyring` 4 adapter (desktop) behind a `SecretSlot` trait; `MigratingSecretKeyStore` moves an old key file into the keychain; file fallback when no keychain. Verified once against the real macOS Keychain (opt-in ignored test) |
| Emergency Kit | ✅ | printable sheet after account creation (Secret Key, blank box for the hand-written master password, restore steps) via Tauri's native `print()`; print-only CSS. **Print layout not yet checked by eye** — please try "Print Emergency Kit" once |
| Desktop MVP | ✅ | everything in the MVP definition works on desktop; remaining items below are polish |

Run it: `cd apps/desktop && npm install && npm run tauri dev`.
UI reviewed visually in the browser demo mode (lock screen, three-pane list, item detail, generator, editor).

Phase 2 backlog (after import/export + settings):
- [x] Re-print Emergency Kit from Settings (master password asked again; the Secret Key stays in the window only while printing).
- [x] TOTP field in the editor + live code display (RFC 6238, SHA-1/256/512, otpauth:// links; the secret never reaches the window).
- [ ] Clipboard via the backend with "concealed" flags (keeps secrets out of clipboard history).
- [x] Lock on system sleep (clock-jump detection, same on every OS). Screen lock is noticed too (see below).

Remaining Phase 2 steps:
3. Screens: onboarding (account creation + Emergency Kit), unlock, vault sidebar, item list,
   item detail/editor, generator, settings (auto-lock, KDF).
4. OS keychain for the Secret Key (macOS Keychain, Windows Credential Manager, Linux Secret Service).
5. Auto-lock (idle, sleep, screen lock) and clipboard auto-clear.

## Phase 3 — Browser extension (MVP: fill + save prompt) — in progress

| Step | Status | Notes |
|---|---|---|
| `password365-match` | ✅ 12 tests, 100 % | registrable-domain matching via the Public Suffix List; levels origin/host/site; no https→http downgrade; IPs/localhost exact; rejects non-web schemes (found & fixed a `mailto:` parsing bug) |
| Browser operations in `password365-app` | ✅ 9 tests | `logins_for_url`, `fill_credentials` (re-checks the match before releasing secrets), `check_submitted` (new / known / password changed), `save_submitted` (create or update with history) |
| Local IPC + native-messaging host | ✅ | `password365-bridge` (JSON protocol, Chrome length-prefixed framing, dispatcher, per-user socket in a 0700 dir + same-uid peer check) and `password365-host` (stateless relay, `app_not_running` when the app is closed). Desktop app serves the socket. Verified with the real binaries: host → running app → `status`/`locked`/`app_not_running` |
| Extension (MV3, TypeScript, esbuild) | ✅ 37 tests, 99.2 % lines | popup (status, logins for site → Fill, generator), content script (login/sign-up detection, fill with input/change events, submit capture incl. Enter/SPA), save/update prompt in a closed shadow root, background router with sender checks; pinned id `ckbcoaigdpfgiabfimlenlepjcloghae`; `npm run install-host` registers the helper (macOS/Linux) |
| Inline menu (1Password-style) | ✅ 55 tests total, 99.5 % lines | icon inside username/password fields; menu under the focused field = extension iframe (`menu.html`, web-accessible) in a closed shadow root, so the page can't read titles or script clicks; logins → Fill, sign-up → "Use suggested password" (fills password + confirmation); closes on Esc / outside click / typing; follows scroll; re-detects SPA forms; background accepts menu actions only from its own `menu.html` frame |
| Unlock from the browser (1Password-style) | ✅ | popup asks for the master password when locked; inline menu shows "Unlock Password365" → opens the popup (password never typed into the page); bridge `unlock` request (`SecretString`); desktop window follows via `vault-status-changed` (also used by auto-lock) |
| Right-click › Password365 › Suggest password | ✅ | browser context menu on any editable field (works where sign-up detection guesses wrong); fills the clicked field + the first empty password field after it (never "current password"); top frame only; app closed → opens the popup |
| Master password strength meter | ✅ | zxcvbn in the core (`password_policy::password_strength`): ≥ 10 chars **and** score ≥ 3/4 ("good"), enforced by the core for app + CLI; live 5-segment meter in onboarding + change-password ("3 more characters needed", "This is a very common password", "Strong enough…"); stale results never enable the button |
| Keyboard + shortcut (1Password-style) | ✅ | ↓ into the menu, ↑/↓/Enter/Esc, filter while typing the username, ⌘⇧L fills (one login) or opens the menu (several); shared `background/actions.ts` for every entry point |
| Encrypted backup & restore | ✅ | core: `Session::backup_to` (SQLite `VACUUM INTO`, owner-only, atomic) + `Store::restore_from` (checks credentials against the backup **before** writing; rejects non-Password365 files); app: `export_backup` / `restore_backup` / `backup_info` (last backup kept in `backup-log.json`, due after 30 days); desktop: Settings › Backup, reminder banner when the vault has items and no backup in 30 days, "Restore from a backup" on first run |
| Sync 4a: merge engine | ✅ | new crate `password365-sync`: Hybrid Logical Clock (drift-guarded), per-field last-writer-wins, password histories merged as sets (superseded/conflicting passwords never lost, conflict flag), deletions that an unseen edit overrides; proptest proves convergence, idempotence, commutativity, associativity (3 devices, skewed clocks); design in `docs/sync-protocol.md` (state-based CRDT instead of the op log in PLAN §4.2) |
| Sync 4b: store integration | ✅ | `password365-vault`: device record (random id, persisted HLC, change sequence; new id after restoring a backup), per-item sync record (stamps + merged history sealed under the vault key), every write atomic via `commit_states`; `changes_since` / `apply_changes` (skips vaults this device lacks, reports password conflicts); delete → **Recently Deleted** (`deleted_items`, `restore_item`, `purge_deleted`); pre-sync items adopted on unlock; two-store end-to-end tests |
| Sync 4c: Recently Deleted | ✅ | 1Password-style: Delete → "Move to Recently Deleted"; sidebar › Recently Deleted lists items with days left, **Restore** (on every device) and **Delete permanently** (confirm); items older than 30 days purged on unlock (`password365-app::recently_deleted`, Tauri `deleted_items` / `restore_item` / `delete_permanently`) |
| Sync 4d: pairing crypto | ✅ | `password365-sync::pairing`: QR invite `PASSWORD365-PAIR:1:…` (one-time 256-bit psk, redacted Debug, strict parser), Noise `XXpsk3_25519_ChaChaPoly_BLAKE2s` via `snow` (only the crypto we use; no `ring`), inviter key pinned by the scanner, 6-digit verification code on both screens, encrypted channel (replay/tamper rejected) |
| Sync 4e: join + peers | ✅ | per-store pairing key + device name; peers with sync progress (`acked_seq` / `received_seq` / `last_sync`, monotonic) and `changes_pending_for`; `join_bundle` (Secret Key + chosen vaults + header + inviter) and `Store::join` (checks the master password before writing, own salt/storage key, only the chosen vaults); restore from backup resets id + key and forgets peers |
| Sync 4f: sync conversation | ✅ | `wire::Message` (Hello / Items / Done / Ack, chunked to the transport limit) + `PeerSync` state machine (peer looked up by pinned key, combined states echoed before Ack, progress recorded); in-memory two-device tests: exchange, incremental, idle ≤ 8 messages, conflicts flagged on both, protocol violations refused |
| Sync 4g: LAN transport | ✅ | new crate `password365-net`: 2-byte length frames, `SecureStream` (Noise over any socket), sync handshake `XX` with pinned peer keys (pairing `XXpsk3` can never be reused for it), `tcp::sync_to` / `accept_sync` with timeouts; tests over real localhost TCP: two-way sync, unpaired device refused, garbage on the socket, pairing over a socket shows the same code on both sides |
| Sync 4h: pairing over the network | ✅ | `password365-net::pairing`: `PairingHost` (listen → QR code → accept → verification code → approve/reject) and `Joining` (connect → same code → receive bundle → `Store::join` → complete); new device pinned on both sides, inviter address remembered; tests: full join + first sync over localhost, wrong password retry, reject/cancel on either side, bad codes |
| Sync 4i: app service | ✅ | `password365-app::devices`: this device (rename), devices list with pending changes / last sync, remove device, `start_pairing` / `approve_pairing` (Secret Key from the keychain) / `join_account` (keychain + unlock; wrong password writes nothing) / `complete_join`, `sync_over` (remembers the peer address); network steps stay outside the service lock; fixed sync port 47480; two `AppService`s pair + sync over localhost in tests |
| Sync 4j: desktop backend | ✅ | Tauri commands: `this_device` / `rename_this_device` / `devices` / `remove_device` / `sync_device`, invite (`invite_device` → QR SVG via `qrcode`, `wait_for_new_device` cancellable, `approve_new_device`, `cancel_pairing`), join (`join_with_code` → `join_wait` → `join_complete`, password retry keeps the bundle); network waits on blocking threads **without** the app lock; background `sync_service`: listens on 47480 only while unlocked **and** paired (opt-in), serves paired devices, auto-syncs every 5 min and on unlock, emits `sync-completed`; tests: invite→approve→join between two app states, cancel, out-of-order steps, serve + Sync now, unreachable skipped, locked vault serves nobody |
| Sync 4k: desktop UI | ✅ | toolbar **Devices**: rename this device, paired devices with "Last synced …" and changes not yet on them, **Sync now** (reports changes and password conflicts), Remove (confirm); **Add a device…**: QR code + pairing code, 6-digit code to compare, choose vaults, "The codes match — add device" / "They don't match", cancel closes the invite; onboarding **Set up with another device**: paste code → compare → wait for approval → master password + device name; window refreshes on `sync-completed`; demo backend supports it |
| Sync 4l: conflict review | ✅ | merges with two concurrent passwords flag the item on each device (local `Review` record); list shows a **Review** badge, the item shows "changed on two devices at the same time … the other one is in the password history" with **Done** (`mark_reviewed`) |
| Sync 4m: LAN discovery | ✅ | mDNS `_password365._tcp` via `mdns-sd` (no TXT, random name each start, only while listening); Sync now falls back to discovery when the remembered address fails and syncs only with the device whose pinned key answers; tests: changed address found and remembered, a different device at a discovered address is not synced, real multicast test (`--ignored`) passes on macOS |
| Sync 4n: out-of-sync alert | ✅ | a paired device missing local changes for over 7 days (or ≥ 50) is `out_of_sync`; banner "Phone is missing N change(s) made here … those changes exist only on this computer" with a Devices button; re-checked after each sync |
| Sync 4o: sync files | ✅ | Devices › **Sync file…** saves `Password365 sync for <device>.qlsync` (owner-only, atomic) that only that device can read; **import a sync file** merges it; tests: encrypted content, idempotent import, reply file acknowledges, wrong recipient / damaged / truncated / tampered / unpaired sender refused with nothing applied |
| Sync 4p | ✅ | real two-computer test (macOS ↔ Windows, 2026-11-11): fixed joining (wait once, reuse a left-over Secret Key, Finish never waits on the network), listener never blocked by outgoing rounds, push 1.5 s after a local change + 60 s rounds |
| Sync 4q | ✅ | revocation: Remove device = grow-only `Revoked` list carried by every sync and `.qlsync` file; all paired devices refuse the removed one; no vault-key rotation (keys never travel after pairing, see sync-protocol.md) |
| Sync 4r | ✅ | join UX: Finish opens the vault at once with a "Syncing with X for the first time…" banner, Copy code button (auto-clearing clipboard), clearer password step |
| Sync 4s | ✅ | master password changes reach every device: the changing device records a `PasswordChange` (HLC stamp + verifier header wrapping a throwaway key); syncs, `.qlsync` files and join bundles carry the newest; others show "changed on Mac — enter the new one", check it against the verifier and re-wrap their store. No password or key travels |
| Manual test in a real browser | ✅ done by you (2026-11-09): save, inline fill, unlock | see `extension/README.md` — I did not install the host manifest into your browser profiles myself |

Plan for the next iterations:
1. Native-messaging host in Rust (`password365-host` binary) speaking a small JSON protocol to the
   extension, forwarding to the running desktop app (local IPC) — the extension never holds keys.
2. WXT + TypeScript MV3 extension: popup (search + fill), content script (login form detection,
   inline fill on user gesture only), save/update prompt after submit.
3. eTLD+1 matching with the Public Suffix List; no autofill in cross-origin iframes.

## Later phases
Phase 3 browser extension · Phase 4 sync · Phase 5 iOS · Phase 6 Android · Phase 7 advanced ·
Phase 8 audit & release — see `PLAN.md` §7.

## Rename: Quillock → Password365 (2026-11-13)
- Product, crates (`password365-*`), CLI (`password365`), browser helper (`password365-host`,
  native-messaging name `app.password365.host`), bundle id `app.password365.desktop`, mDNS
  `_password365._tcp`, pairing codes `PASSWORD365-PAIR:1:`.
- Format v1 labels stay `quillock/v1/…` (see `docs/crypto-spec.md`), so existing vaults keep opening.
- First start copies the old app folders (`io.quillock.desktop`) into the new ones (old copy kept)
  and, on Windows/Linux, moves the Secret Key to the new keychain entry. On macOS the unlock screen
  asks for the Secret Key once (reading the old Keychain entry would show a password dialog).
- After pulling: re-run `npm run install-host` in `extension/` (helper renamed).

## 1Password parity, part 1 (2026-11-13)
- Sidebar: **Favorites** and a **Tags** section (tag + item count), like 1Password; star in the item header
  adds/removes a favorite (`set_favorite`, synced like any field). `tags` command lists tags in use.
- Item list: **All Categories** filter (only categories present) and sort by **Title** / **Date Modified**.
- **Archive** (item format v2, see `docs/sync-protocol.md`): Archive in the sidebar, "Archive" on an item,
  "Restore" on an archived one; archived items leave the list, counts, tags and browser filling; synced.
  Old records and v1 peers' messages are read and upgraded.
- **Categories** (1Password's 21): Login, Password, Secure Note, Credit Card, Identity, API Credential,
  Bank Account, Crypto Wallet, Database, Driver License, Email, Medical Record, Membership, Outdoor License,
  Passport, Rewards, Social Security Number, Software License, SSH Key, Server, Wireless Router. Field
  templates live in `password365-model/src/item_template.rs` (one place; the UI reads them via `categories`);
  values are stored as the item's custom fields, concealed ones only leave the core when revealed
  (`reveal_field`). "+ New item" opens "What would you like to add?" (search, 6 tiles, the rest as a list);
  the editor shows each category's fields with fitting inputs (date, month, e-mail, phone, multi-line,
  concealed). No item-format change was needed (new kinds are appended).
  Later done: Document and attachments, per-field sync of custom fields (format v3), and the CSV export
  adds a login's own fields to the end of its note (Chrome's format has no other place for them).
- **Sections and field types** (like 1Password's editor): category fields sit under their sections
  (Identity: Identification / Address / Internet Details; card: Contact Information / Additional Details; …).
  Every section ends with "+ add another field" (Text, URL, Email, Address, Date, One-Time Password,
  Password, Phone); "+ add more" starts a named section; added fields can be renamed and removed. The item
  view groups fields by section and shows a one-time password field as its live code (`totp_code` with a
  label; the secret never reaches the window). Field names must be unique per item. Section and type of
  fields that differ from the template are kept in one internal custom field
  (`password365-app/src/field_layout.rs`), so no format change: it syncs with the fields.
  "Sign in with" fields point at another login (shown as a link). Attachments: see below.
- **Watchtower** (`password365-app/src/watchtower.rs`), all on this device: score 0–1000 (mean zxcvbn
  score of the passwords ×250, a reused password counts 0) with rating, strength bar (strong / good / weak),
  weak passwords (zxcvbn < 3), reused passwords (groups), unsecured websites (http://), items expiring within
  30 days or expired (fields "expiry date" / "expires"). Archived items are left out. Sidebar › Watchtower;
  "Show items" lists them and opens one.
  Not done: breach checks (Have I Been Pwned needs the internet: would be opt-in, k-anonymity),
  "inactive two-factor authentication" (needs a bundled list of sites that support 2FA).
- **Auto-type** (KeePass-style): `password365-app/src/auto_type.rs` plans keystrokes from a sequence
  (`{USERNAME}{TAB}{PASSWORD}{ENTER}` default; `{TOTP}`, `{TITLE}`, `{URL}`, `{S:field}`, keys,
  `{DELAY 500}` pause, `{DELAY=50}` per-key delay; validated on save) and matches windows (patterns with `*`,
  else the window title contains the item title). Settings are two custom fields of the item, so they sync.
  Desktop (`auto_typer.rs`, enigo + global-shortcut + active-win-pos-rs): **Ctrl+Alt+A** (Control+Option+A on
  a Mac) types the one matching item into the window in front; none/several → Password365 comes forward to
  choose. Item › **Auto-type** steps aside (hide on macOS, minimize elsewhere) and types into the previous
  window. Editor › Auto-type section (sequence + windows).
  macOS: needs Accessibility permission (asked on first use) to type; window titles of other apps need
  Screen Recording permission, without it the program's name is matched instead.
  **Not yet tried on real programs** (only unit-tested).
- **Profile** (1Password-style): sidebar › Profile chooses the default identity and payment card (the only
  one of its kind is used when nothing is chosen; the choice is kept on this device in `profile.json`).
  Extension: address and payment fields are recognised by their autocomplete names, or by name/label/placeholder
  in English, Portuguese and Spanish (`content/profileFields.ts`); they get the Password365 icon, and the menu
  offers "Identity › Me" or "Payment card › Visa •••• 4242". Values come from the app only on that click
  (bridge `profile` / `profile_fill`), keyed by autocomplete names; card details only on https (or localhost)
  pages; selects are matched by value or text; the expiry date is shaped as the page expects (MM/YY, MM/YYYY,
  month input, separate month/year). Not tried on real shops yet.

## Global: languages and countries (2026-11-14)
- **Languages** (`i18n/`, see `i18n/README.md`): every text of the desktop app and the extension comes
  from `i18n/locales/<tag>/<area>.json`; English, Português (Brasil) and Español ship. Settings ›
  Language (or the system's). Adding a language = adding a folder; tests check keys and placeholders.
  Category, field and section names are translated for display only (stored labels never change).
- **Countries** (`templates/`, see `templates/README.md`): category fields live in
  `templates/categories.json` (embedded in the core at build time); `templates/regions/<CODE>.json`
  replaces a category's fields for a country, with masks, choices, named checks (cpf, cnpj, luhn, iban)
  and autofill tokens. Brazil ships: CPF, RG, órgão emissor, CNH, CNPJ, CEP, logradouro, número,
  complemento, bairro, UF; CNH item. Items remember their country (editor: "Country or region"; new
  items follow the app language's country); switching keeps values.
- **Filling**: values are keyed by what forms call them (template `autofill`); the extension recognises
  boxes with the patterns of `templates/autofill.json` + each country's (no patterns in code), fills an
  identity's own fields into boxes with the same label, types digits only when a formatted value does not
  fit, and offers every identity / card (default first).
- **Migrate your data** (Import & export): a 1Password-like picker (search; My browser / A password
  manager / Somewhere else) with export steps for Chrome, Safari, Firefox, Edge, Brave, Bitwarden,
  LastPass, Apple Passwords, Dashlane, KeePass, KeePassXC, RoboForm, 1Password, Proton Pass, NordPass,
  spreadsheets and any CSV. Sources and CSV column names are data (`templates/imports.json`); folders
  become tags, favorites stay, notes-only rows become secure notes. KeePass .kdbx too (below).
- **Settings like 1Password's** (sections on the left): General (start at login via the Tauri autostart
  plugin, default vault for new items, open to All items / Favorites / Watchtower), Appearance (theme
  system/light/dark, density, interface zoom, categories and tags in the sidebar), Security (auto-lock,
  lock on sleep, clipboard clearing 30 s–5 min or never, Emergency Kit, master password), Backup,
  Advanced (language), About. All stored per device in settings.json. The sidebar can list categories.
- **Item menu (⋮)**: add/remove favorites, Move… to another vault, Duplicate (fields copied, not the
  password history), Archive, Delete.
- **Quick Access** (Shift+Cmd+Space on a Mac, Ctrl+Shift+Space elsewhere): a small always-on-top
  window over any program; search, ↑↓, Enter copies the password, Cmd/Ctrl+C the username,
  Cmd/Ctrl+Alt+C the one-time password, Cmd/Ctrl+O opens the item in the app; hides after a copy, on
  Escape or when it loses focus; when locked it offers to open the app. Global shortcuts live in
  `src-tauri/src/shortcuts.rs`.
- **SSH agent** (Settings › Developer, off by default; crate `core/password365-ssh`): serves the Ed25519
  keys of SSH Key items to ssh and git on `~/.password365/agent.sock` (folder 0700) or the Windows pipe
  `\\.\pipe\password365-ssh-agent`; asks once per key per unlock ("Allow SSH key?"), waits for an
  unlock when locked, never exports private keys. Passphrase-protected keys use a "passphrase" field.
  Proven against real OpenSSH (`ssh-add -L`, `ssh-keygen -Y sign`). Not yet: RSA and ECDSA keys (their
  Rust crypto stacks have open timing advisories; cargo-deny refuses them).
- **Wi-Fi QR codes** on Wireless Router items; **own item icons** (Choose new icon…).
- Blocked on signing (QUESTIONS 19): Touch ID, Windows Hello, macOS AutoFill.
- **Vulnerable passwords** (Settings › Privacy, off by default; the only feature that uses the internet):
  Watchtower › Check now asks haveibeenpwned.com with the first 5 characters of each password's SHA-1
  (k-anonymity, padded answers); the vault is read under the lock, the network part runs in the
  background. HTTPS via rustls (deny.toml now allows ISC and CDLA-Permissive-2.0).
- **More countries** (data only): United States, Portugal, Spain, Argentina, Mexico (documents, address
  parts, states/provinces, masks, form words); checks nif_pt, dni_es, cuit. The extension tries card
  patterns before identity ones whatever file they come from.
- **Keyboard shortcuts** (Settings › General): Quick Access, auto-type and Lock Password365 are global
  shortcuts the user records (or clears); re-registered on save; two actions cannot share one.
- **1Password full export (.1pux)**: Migrate › 1Password takes the 1PUX file and imports every category
  (logins, cards, identities, notes, SSH keys …) with sections, tags, favorites and the archive;
  unreadable one-time codes are kept as a field; items already there (same category, title, username)
  are skipped; documents (attachments) are not imported yet.
- **Country is its own setting** (Settings › General › Country or region): automatic (the system's
  region), a country with its own fields, or "another country" (international fields). New identities
  follow it, never the interface language — Brazilian fields with the app in English work.
- **SSH Key items**: "key type" is a list (Ed25519, RSA, ECDSA, DSA); for Ed25519 keys the app fills the
  key type, public key and fingerprint from the private key when saving.
- **Guessing the master password**: wrong passwords in a row make the next try wait (2 free, then 5 s,
  15 s, 30 s, 1 min, 2 min, 5 min, then 15 min each), stored on disk so restarting does not reset it;
  after 10 in a row the computer forgets its Secret Key and the next unlock needs it typed from the
  Emergency Kit. Applies to unlock (app and browser extension), changing the master password, revealing
  the Secret Key, adopting a password changed elsewhere and joining a device. Copies of the vault taken
  elsewhere rely on the Secret Key (128 bits) + Argon2id instead.
- **Category icons** (`icons/`, see `icons/README.md`): 39 own SVG icons in one style (bold rounded
  strokes, a coloured tile with a white glyph); every category has its icon and colour (lists, sidebar,
  new-item picker, item header). Items can pick a gallery icon in one of 8 colours (stored as
  `builtin:<name>:<colour>`) or keep using a picture.
- **Copying secrets** goes through the app (arboard): hidden from Windows clipboard history, cloud
  clipboard and clipboard monitors, and marked concealed on macOS; cleared after the chosen delay unless
  something else was copied.
- **Reordering fields** in the editor: drag a field's ≡ handle onto another field (it takes that
  field's section), or focus the handle and press ↑ ↓.
- **Bitwarden JSON export** (unencrypted): Migrate › Bitwarden imports logins, cards, identities, notes
  and SSH keys with custom fields (section "More"), folders as tags and favorites; encrypted exports are
  refused with how to export again.
- **Locks when the screen locks** (same setting as sleep): checked every 10 s through what each system
  exposes, no native code — macOS `ioreg` IOConsoleLocked, Windows the LogonUI.exe lock screen, Linux
  `loginctl` LockedHint.
- **Edge and Firefox**: `npm run build` also writes `dist-firefox/` (background scripts, add-on id
  `extension@password365.app`); `npm run install-host` registers the helper for Chrome, Edge, Brave,
  Chromium and Firefox on macOS, Linux and Windows (per-user registry keys).
- **KeePass / KeePassXC (.kdbx)**: Migrate › KeePass opens the database with its password (not stored):
  folders become tags, extra fields go to a "More" section (protected ones hidden), one-time passwords
  (`otp`, TimeOtp) come along, the recycle bin is skipped. Key files are supported (with or without a
  password). Not supported: hardware keys (YubiKey challenge-response).
  (`keepass` 0.14 accepts aes "<0.9.3"; Cargo.lock pins its aes to 0.9.2.)
- **Format v3, custom fields synced one by one**: each custom field (and each attachment) is its own
  last-writer-wins entry with its own stamp, removals included; edits of different fields on two devices
  both survive. Older items, sync states and messages (v1, v2) are still read and upgraded. Every device
  must run this version before syncing again.
- **Attachments and the Document category**: any item can hold files (up to 50 MB each); a Document is a
  file plus notes. Each file is encrypted with its own random key, kept only inside the encrypted item;
  the encrypted bytes live in their own SQLite table (not loaded on unlock, copied by backups). The item
  view lists files with Open (a temporary owner-only copy opened with the system's program, deleted after
  10 minutes and on start), Save… and Remove; files of items in Recently Deleted stay until the item is
  purged. Files sync between paired devices: after merging items, each side asks for the files it
  lacks, which are sent in pieces inside the encrypted channel and checked against the item's
  digest. `.qlsync` files carry them too.
- **1Password `.1pux` attachments**: Documents (category 006) import as Documents, and files attached
  to any item come along (read from `files/<documentId>__<name>` in the zip). Re-importing does not
  attach files twice. Checked against a real 1Password export.
- **Passkeys** (like 1Password): the extension answers `navigator.credentials.create/get` on https pages
  (and localhost). A login holds one passkey (ES256, attestation "none", counter 0 like other synced
  passkeys) in an internal field: synced per field, never shown in forms, never exported, kept when the
  login is edited. The site comes from the tab's real address and the core builds `clientDataJSON`;
  the relying-party id must be the page's host or a parent domain that is not a public suffix. The
  user confirms each save / sign-in in the extension's own frame (top right of the page); "Use another
  device or method", a locked app or no passkey for the site hand over to the browser. The item view
  shows the passkey with Remove. Verified with an independent server library (py_webauthn 2.5.2). Tested by hand on webauthn.io (Chrome, 2026-11-14): register and sign in both work.
  A locked app is unlocked from the prompt (the popup) and the request carries on. "Conditional"
  sign-ins (passkeys offered in the username field's list) show this vault's passkeys in the inline
  menu of that field (`autocomplete="… webauthn"`); without any, or when locked, the browser's list is
  used; when the page gives the request up, the menu stops offering them.
- **CXF export** (FIDO Credential Exchange Format 1.0): Import & export › "Export CXF…" writes one
  owner-only JSON file (plaintext, same warning as CSV) with every active and archived item: logins
  (basic-auth), passkeys (PKCS#8 key, checked with Python `cryptography`), one-time passwords
  (standard Base32), cards, notes, tags, favorites, websites, and all other fields as custom fields by
  section. Attached files stay in the vault (CXF moves them only through the live exchange protocol).
- **CXF import**: Migrate › "Credential exchange file (CXF)" reads CXF 1.x files from other managers:
  logins with passkeys (P-256 PKCS#8 keys; others are listed as not imported) and one-time passwords
  (non-default codes kept as `otpauth://`), cards, notes, custom fields by section, and every other
  kind (Wi-Fi, documents, addresses, API keys…) as fields of the nearest category. Attached files and
  SSH private keys are not in such files; the summary says so. Tested by exporting from one account,
  importing into another and signing in with the imported passkey (signature checked against the
  original registration).
- **iOS app, M1 started** (`docs/ios-plan.md`, `apps/ios/`): `password365-ffi` (UniFFI 0.32.2) exposes
  status / create / unlock / lock / items / item / reveal / TOTP, with the Secret Key kept by the app
  (iOS Keychain, this device only) through a Swift-implemented `KeyKeeper`; tests in Rust. The
  generator lives outside the core workspace (its `basic-toml` is unmaintained, cargo-deny). The
  SwiftUI app (create account → Secret Key, unlock incl. Secret Key on a new device, searchable list,
  item view with reveal / copy, live one-time codes; copies expire after 90 s and stay on the phone;
  locks when backgrounded) runs in the simulator; a UI test walks the first run (create account →
  Secret Key → new login → reveal → lock → unlock) with a screenshot per screen. Found and fixed on
  the way: the Secret Key screen was skipped; iOS offered to save typed passwords (even the master
  password) into iCloud Keychain — password fields are no longer marked as passwords and forms are
  emptied before they close. Texts come from the shared `i18n/` catalogs.
- **iOS app, M2**: every category from its template (country templates with a region picker that
  keeps typed values), editor with labels above values, password generator (password / passphrase /
  PIN), one-time password setup, websites, notes, tags, favorites; item menu with Edit, Favorite,
  Archive / Restore and Delete (to Recently Deleted); list filtered by All / Favorites / Archive.
  The editor loads secrets in clear only when Edit is tapped and wipes them before closing. iPad
  enabled. UI tests: first run, a card from the template edited / starred / archived / deleted, and
  the generator filling a new login (all green). Found and fixed: the editor crashed on save.
- **iOS app, M3**: "Set up with another device" on the phone — scan the computer's QR code (VisionKit)
  or paste it, compare six digits, then master password and device name; the Secret Key goes to the
  Keychain. Devices screen (this device's name, paired devices, last sync, changes not on them,
  Sync now); the phone syncs after unlocking and on pull-to-refresh (iOS allows the network only
  while the app is open). Local-network and camera permission texts come from `mobile.json` in every
  language (InfoPlist.strings generated at build). Tests: a Rust end-to-end join + sync both ways;
  a UI join test against a stand-in computer (`apps/ios/test-join.sh`). The phone refused the
  stand-in's test-only weak Argon2 settings, as it should.
  Not yet: finding the computer again by Bonjour when its address changes; `.qlsync` through the
  share sheet.
- **iOS app, M4**: Face ID / Touch ID unlock — after a master-password unlock, the store key is
  wrapped by a random device key (`password365-vault::quick_unlock`, bound to the store and to the
  current password's salt, with its creation time); the key lives in the Keychain behind
  `.biometryCurrentSet` (this device only). The master password is needed again after 14 days, after
  it changes, or when the phone's biometry changes; each password unlock renews it. Settings screen:
  Face ID, lock after leaving the app (immediately / 1 / 5 / 15 / 60 min, 1 by default on a phone),
  clipboard expiry (30 s / 90 s / 3 min / never), country. A privacy cover hides the vault in the app
  switcher. Tests: vault, app and FFI unit tests; a UI test (`apps/ios/test-faceid.sh`) enrolls a
  simulator face, turns Face ID on, locks and unlocks without typing.
- **iOS app, M5 — AutoFill**: a Credential Provider extension (passwords, one-time codes on iOS 18+,
  passkeys) sharing the vault through the App Group `group.app.password365` and the Secret Key /
  Face ID key through its Keychain group. The extension opens the vault only with Face ID / Touch ID
  (no Argon2 in its small memory). The app registers its logins and passkeys (site and account only)
  with `ASCredentialIdentityStore` after unlocking, after changes and when it comes back to the
  screen. Passkeys use new platform variants in `password365-webauthn` (the system checks the site and
  builds the client data; we sign over its hash). Logins are filled only on their own sites. Tests:
  Rust (platform passkeys, autofill FFI); a UI test (`apps/ios/test-autofill.sh`) enables Password365
  in iOS Settings, then Safari fills a local sign-in page with the right username and password after
  Face ID. Passkey AutoFill is covered by unit tests only (it needs a real site's associated domain).
  On a real iPhone, AutoFill and the App Group need the paid Apple Developer account.

- **iPhone app moved to its own private repository** (`remofurlanetto/password365-ios`, with its
  history): the paid mobile app stays closed while this repository (Rust core, desktop app, browser
  extension) can be opened. That repository checks this one out as its `upstream` submodule for the
  core (`password365-ffi` stays here, with its tests and the `fake_mac` example) and the texts
  (`i18n/locales/*/mobile.json` stays here, global by config). The `apps/ios/` paths above now
  live at the root of that repository. Before this repository is made public its history must be
  rewritten without `apps/ios/`, since every earlier commit still contains it.
- **Website** (`site/`, Astro 7.4.4, static): password365.app in English, Portuguese and Spanish
  (texts in `i18n/site/`), slogan "Not your server, not your passwords." — home (why not the
  cloud, sync without a server across many devices, features, security, imports, platforms,
  FAQ), Security, Privacy policy, Support, Download (links "coming soon" until builds exist),
  404. Screenshots from the App Store set; logo and icons from `branding/` and `icons/`.
  Tests: every language has English's keys and placeholders, every key the pages use exists.
  To do: hosting on Cloudflare Pages, a real contact address (`site/src/config.ts`).
- **Visible Quillock names renamed** (2026-11-16): the Secret Key now reads `P365-XXXX-…`
  (`Q1-` keys are the same keys and still parse; prefix not in the checksum or the derivation,
  published AUK vectors unchanged but for the display text); backups are `.p365backup` and sync
  files `.p365sync` (the desktop still opens `.qlbackup` / `.qlsync`, saves only the new names).
  Internal labels (`quillock/v1/…`, `.qlvault`, `QLSYNC1`) stay: format v1, never shown.
- **Releases and updates (desktop)** (2026-11-16):
  - `.github/workflows/release.yml`: macOS (universal), Windows (NSIS + MSI) and Linux (AppImage +
    deb) installers; for a `v*` tag, a GitHub release with stable file names, the signed update
    bundles, `latest.json` and the version's `CHANGELOG.md` section
    (`scripts/release/latest_json.py`, tested). Without the signing key it only builds installers.
    How to release and the one-time key setup: `scripts/release/README.md`.
  - Updates in the app (`tauri-plugin-updater` behind the app's own `update_check` /
    `update_install`): off until the first-run question is answered; automatic = shortly after
    start, then daily; Settings › About › Check for updates; the notice shows what changed and
    offers "Skip this version" (`skipped_update`). Every update must carry Password365's signature.
  - The browser extension's helper ships inside the app (`bundle.externalBin`, built by
    `apps/desktop/scripts/sidecar.mjs`) and the app registers it with every installed browser at
    start (`browser_host.rs`; HKCU registry on Windows) — no more `npm run install-host` for users.
  - Settings › About › Help and feedback: bug, idea, translation, private security report, email
    (fixed addresses in `help.rs`).
  - Public-repository files: README, CONTRIBUTING (translations), SECURITY, CHANGELOG, issue forms,
    pull-request template.
