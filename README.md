# Password365

**An offline-first, open-source password manager.** No cloud, no account server, no telemetry.
Your devices sync only with each other — over local Wi-Fi, Bluetooth, or an encrypted file you move
yourself.

> Status: **MVP code complete** — Rust core + CLI, desktop app (Tauri), and browser extension
> (fill + save prompt). Next: manual browser testing, then device-to-device sync and mobile apps.
> See [`PLAN.md`](PLAN.md) and [`docs/PROGRESS.md`](docs/PROGRESS.md).
> Not yet audited — do not rely on it for real secrets yet.

## Security at a glance
- Master password **+ 128-bit Secret Key** (two-secret key derivation, like 1Password).
- Argon2id (256 MiB) → HKDF-SHA-256 → XChaCha20-Poly1305.
- The store file reveals nothing: titles, URLs, usernames and vault names are encrypted, records are
  padded, deleted data is overwritten.
- Details: [`docs/crypto-spec.md`](docs/crypto-spec.md), [`docs/threat-model.md`](docs/threat-model.md).

## Try the CLI

```sh
cd core
cargo build --release
./target/release/password365 init                      # creates store + prints your Secret Key
./target/release/password365 generate --passphrase
./target/release/password365 item add --title GitHub --username me --url https://github.com --generate
./target/release/password365 item list
./target/release/password365 item show github --reveal
./target/release/password365 import ~/Downloads/Chrome\ Passwords.csv
./target/release/password365 export chrome.csv         # plaintext, for importing into Chrome
```

Run `password365 --help` for all commands.

## Repository layout

```
core/
  password365-crypto/     primitives, Secret Key, key derivation, key wrapping
  password365-model/      vaults, items, fields, password history
  password365-generator/  passwords, passphrases (EFF wordlist), PINs
  password365-vault/      encrypted SQLite store, sessions
  password365-import/     browser CSV import/export, duplicate detection
  password365-app/        UI-agnostic application service (used by every frontend)
  password365-match/      website matching (Public Suffix List)
  password365-bridge/     extension ↔ app protocol and local socket
  password365-host/       native-messaging helper started by the browser
  password365-cli/        `password365` command-line tool
apps/desktop/          desktop app (Tauri 2 + React/TS)
extension/             browser extension (MV3, TypeScript)
docs/                  specs, threat model, progress, open questions, ideas
```

## Development

```sh
cd core
cargo test --workspace                 # unit + end-to-end tests
cargo clippy --workspace --all-targets
cargo llvm-cov --workspace             # coverage (project minimum: 90 %)
```

Desktop app: `cd apps/desktop && npm install && npm run tauri dev`. On macOS, dev builds are
signed automatically with your first "Apple Development" certificate (override with
`PASSWORD365_SIGNING_IDENTITY`) so the Keychain recognises every rebuild and never prompts — see
`apps/desktop/src-tauri/scripts/macos-dev-sign.sh`. Release builds: Developer ID + notarization.

Rules: test-driven, ≥ 90 % line coverage, small single-purpose modules, no duplicated logic,
`unsafe` forbidden.

## License

[GPL-3.0-or-later](LICENSE). The EFF wordlist (`core/password365-generator/data`) is CC BY 3.0 US.
