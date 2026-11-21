# Changelog

What changed in each version of Password365 (desktop app, browser extension and core). The
release page and the app's update notice show the section of the new version.

## [Unreleased]

## [0.1.2] — 2026-11-21

### Fixed
- Erasing the vault (and starting a new one) no longer fails when the macOS Keychain entry with the
  Secret Key was made by another build of the app ("Invalid attempt to change the owner of this
  item"). That entry is left alone and ignored, and the key of the new vault is kept in the app's
  owner-only key file.

### Changed
- The lock screen and the erase screen have the logo and more room between their parts.
- The browser extension's popup has the logo and clearer messages: the app is not open (with a link
  to get it), the vault is locked (with a Show/Hide button for the master password), logins with
  their initial, and the password generator in its own card.

## [0.1.1] — 2026-11-21

### Added
- Erase this vault: from the lock screen ("Forgot your master password? Erase this vault…") and from
  Settings › Advanced. It works without the master password, asks you to type a confirmation word,
  and brings you back to the first-run screens, where you can start a new vault, join another device
  or restore a backup.
- The QR code that adds a device is now a link: scanning it with the phone's own camera opens
  Password365 on the phone and connects right away.

### Changed
- The app's address for update checks moved with the project to the `concriz-group` organization on
  GitHub (the old address keeps redirecting).

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
