# Password365

**Not your server, not your passwords.**

Password365 (also written Password 365) is an open-source password manager that keeps your passwords, passkeys and one-time codes in an
encrypted vault **on your own devices**. Your computer, phone and tablet sync **directly with each
other** — on your network or with a file you carry — never through a server. No account, no cloud,
no tracking.

Built to work 365 days a year: offline, with no account that can expire and no server whose outage can lock you out.
Compare it with [1Password and Bitwarden](https://password365.app/1password-bitwarden-alternative/) or
[KeePassXC](https://password365.app/keepassxc-alternative/).

[Website](https://password365.app) · [Download](https://github.com/concriz-group/password365/releases/latest) ·
[Security](docs/crypto-spec.md) · [Report a bug](https://github.com/concriz-group/password365/issues/new/choose)

> **Status:** early releases. Password365 has not had an independent security audit yet.

## Features

- **Everything a password manager does:** logins, passkeys, one-time codes (TOTP), credit cards,
  identities, secure notes, SSH keys, documents and 20+ categories with the fields of your country;
  Watchtower (weak, reused and old passwords); a password generator; attachments.
- **Fills in for you:** a browser extension for Chrome, Edge and Firefox (fill, save, passkeys),
  Quick Access (a shortcut that opens your vault over any app), auto-type and an SSH agent.
- **Syncs without a cloud:** pair a device once by scanning a QR code; after that devices exchange
  changes on the same network, or with an end-to-end encrypted `.p365sync` file.
- **Bring your data, take it with you:** import from 1Password, Bitwarden, KeePass/KeePassXC,
  LastPass, Dashlane, Proton Pass, Chrome, Safari, Firefox, Edge and CSV; export to CSV or the
  FIDO Credential Exchange Format (CXF).
- **In your language:** English, Português, Español — [add yours](CONTRIBUTING.md#translations).

## Install

Download the newest version for your system from the
[releases page](https://github.com/concriz-group/password365/releases/latest):

| System | File |
|---|---|
| macOS (Apple silicon and Intel) | `Password365-macOS.dmg` |
| Windows 10 and 11 | `Password365-Windows-setup.exe` (or `.msi`) |
| Linux | `Password365-Linux.AppImage` or `.deb` |
| Browser extension | `Password365-extension-Chromium.zip` (Chrome, Edge, Brave) or `-Firefox.zip` |

The macOS app is signed and notarized by Apple. The Windows installer is not code-signed yet, so
SmartScreen may warn the first time ("More info › Run anyway"). Until the extension is in the
browser stores, unzip it and load it as an unpacked extension (Chrome: `chrome://extensions` ›
Developer mode › Load unpacked); the app registers its helper with your browsers by itself.

The app can check for new versions once a day (it asks the first time you open it); every update
is signed, and the app refuses one that is not. Password365 for iPhone and iPad is built on the
same core and is coming to the App Store.

## Security

- Your vault is unlocked by your master password **and** a 128-bit Secret Key created on your
  device (two-secret key derivation): a copy of the vault cannot be attacked with password
  guesses alone.
- Argon2id (256 MiB) → HKDF-SHA-256 → XChaCha20-Poly1305; every record is bound to its vault,
  item and version. The vault file reveals nothing: titles, websites and usernames are encrypted.
- Devices pair in person (QR code) and sync over Noise (X25519) channels; sync files are encrypted
  to one paired device.
- The only features that use the internet — the breached-password check (Have I Been Pwned,
  k-anonymity) and update checks — are off until you turn them on.

Details: [crypto specification](docs/crypto-spec.md) · [sync protocol](docs/sync-protocol.md) ·
[threat model](docs/threat-model.md) · [security policy](SECURITY.md) (how to report a
vulnerability privately).

## Build from source

```sh
# The Rust core and the command-line tool
cd core && cargo test --workspace && cargo build --release

# The desktop app (Tauri 2 + React)
cd apps/desktop && npm install && npm run tauri dev

# The browser extension
cd extension && npm install && npm run build    # then load extension/dist unpacked
```

Requirements: Rust (stable), Node.js 22; on Linux the Tauri system libraries
(`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`). More in
[CONTRIBUTING.md](CONTRIBUTING.md).

```
core/            Rust: crypto, vault, sync, import/export, the app service, the CLI
apps/desktop/    desktop app (Tauri 2 + React/TypeScript)
extension/       browser extension (Manifest V3)
i18n/            every text, one folder per language
templates/       item fields per country
site/            password365.app
docs/            specifications, threat model, progress log
```

## Contributing and support

Bug reports, ideas and translations are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). In the
app: Settings › About › Help and feedback. Questions: hello@password365.app.

## License

[GPL-3.0-or-later](LICENSE). The EFF wordlist (`core/password365-generator/data`) is CC BY 3.0 US;
third-party licences are listed in the app (Settings › About › Third-party licenses).
