# Changelog

What changed in each version of Password365 (desktop app, browser extension and core). The
release page and the app's update notice show the section of the new version.

## [Unreleased]

## [0.1.5] — 2026-11-23

### Added
- **Pairing works in every direction.** A new computer or phone now shows its own QR code on the
  "Set up with another device" screen, and a device that has the vault scans it (Devices › Add a
  device on the phone; on a computer, paste the code the new device shows). Showing the code from
  the device that has the vault, and scanning or pasting it on the new one, still works too.
- A login saved for one of a brand's country sites is offered on the others (`amazon.com` on
  `amazon.es`, Google, Microsoft, Apple, eBay, PayPal and more; the list is
  `templates/equivalent-domains.json`). The first time it is used on such a site, the app asks for a
  confirmation and then remembers the site (kept sealed in the vault, on this device only).
- A website written with a wildcard ending (`amazon.*`) on a login matches that exact name under any
  domain ending, and never look-alikes.

### Changed
- The extension's popup and inline menu mark a related-site login ("First time with this login on
  this site") and ask once before filling it.

### Security
- Updated the `url` library (RUSTSEC-2026-0592).

## [0.1.3] — 2026-11-21

### Fixed
- Texts, fields, buttons and links no longer touch each other on the first-run screens (create
  vault, set up with another device, restore a backup) and in the Devices and Settings panels.

### Added
- Add a device from the iPhone: the phone shows the QR code, and a computer set up with "Set up with
  another device" joins by pasting the pairing code or link. This version already reads both.

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
