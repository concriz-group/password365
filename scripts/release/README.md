# Releasing the desktop app

## Once: the update signing key

Every update is signed; the app accepts only updates signed with this key (its public half is in
`apps/desktop/src-tauri/tauri.conf.json`, `plugins.updater.pubkey`). **Losing the private key means
installed apps can no longer be updated**, so keep it, with its password, in a safe place (your
password manager, an offline backup).

1. Create it on your own computer (it asks for a password):
   ```sh
   cd apps/desktop && npx tauri signer generate -w ~/.tauri/password365-updater.key
   ```
2. Put the public key (`~/.tauri/password365-updater.key.pub`, one line) in `tauri.conf.json` as
   `plugins.updater.pubkey`.
3. Give the private key and its password to GitHub Actions as repository secrets:
   ```sh
   gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/password365-updater.key
   gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD     # paste the password when asked
   ```

## Every release

1. In `CHANGELOG.md`, move the "Unreleased" items under a new `## [x.y.z] — date` heading.
2. Set the same version in `apps/desktop/src-tauri/tauri.conf.json` (and `apps/desktop/package.json`).
3. Commit, then tag and push: `git tag vx.y.z && git push origin main vx.y.z`.

`.github/workflows/release.yml` builds macOS (universal), Windows and Linux installers, signs the
update bundles and publishes a GitHub release with the changelog section, the installers under
stable names (`Password365-macOS.dmg` …) and `latest.json`, which installed apps read from
`releases/latest`. Running the workflow by hand (Actions tab) only builds, to try things out.

## macOS signing and notarization (Developer ID)

Without these secrets the macOS installer is unsigned (a test build only; a `v*` tag refuses to publish without them).

1. A **Developer ID Application** certificate in your Keychain Access (developer.apple.com › Certificates).
   Export it with its private key as a `.p12` (Keychain Access › right click › Export), with a password.
2. An App Store Connect API key (Users and Access › Integrations; the `.p8`, its Key ID and the Issuer ID).
3. Give them to GitHub Actions:
   ```sh
   base64 -i DeveloperID.p12 | gh secret set APPLE_CERTIFICATE
   gh secret set APPLE_CERTIFICATE_PASSWORD
   gh secret set APPLE_SIGNING_IDENTITY       # "Developer ID Application: Your Name (TEAMID)"
   gh secret set APPLE_API_KEY                # the Key ID
   gh secret set APPLE_API_ISSUER             # the Issuer ID
   gh secret set APPLE_API_KEY_P8 < AuthKey_XXXX.p8
   ```

## Not yet

- Windows: code signing (SmartScreen warns about unsigned installers).
