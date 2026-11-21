# Changelog

What changed in each version of Password365 (desktop app, browser extension and core). The
release page and the app's update notice show the section of the new version.

## [Unreleased]

## [0.1.0] — 2026-11-21

First public version.

### Added
- An encrypted vault on your devices: master password plus Secret Key, Argon2id and
  XChaCha20-Poly1305; Emergency Kit and backups.
- Logins, passkeys, one-time codes, credit cards, identities, secure notes, SSH keys, documents and
  more than 20 categories, with the fields of your country; attachments, tags, favorites, archive.
- Watchtower: weak, reused and old passwords, and an optional breached-password check.
- Browser extension for Chrome, Edge and Firefox: fill, save, generate, passkeys.
- Quick Access, auto-type, an SSH agent and global shortcuts.
- Sync between your devices without a server: QR-code pairing, sync on the same network, and
  `.p365sync` files.
- Import from 1Password, Bitwarden, KeePass/KeePassXC, browsers and CSV; export to CSV and CXF.
- Update checks (off until you choose), with what changed and "Skip this version".
- English, Português and Español.
- The Emergency Kit is also kept inside your vault as a PDF document (created with a new
  account; Settings › Security › Emergency Kit › Save in the vault), so you can see, print or
  share your Secret Key from any unlocked device.

### Security
- The remaining low findings of the outside review: a sealed integrity sum, a message-bound menu
  token for the extension, an attachment allow-list, and passkeys claiming user verification only
  when it really happened.
