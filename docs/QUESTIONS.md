# Open questions for the project owner

Decisions taken autonomously with a default are marked **(default applied)** — change them any time.

1. ~~**Name**~~ — **Resolved (you, 2026-11-13): the app is Password365, domain `password365.app`
   (registered).** Renamed from Quillock everywhere. Please rename the GitHub repo to
   `remofurlanetto/password365` (Settings › Rename; old links keep redirecting).
2. **Trademark** — "365" means every day of the year; a quick search found no password manager with
   this name. Optional: a trademark check before public launch.
3. **License** — GPL-3.0-or-later for everything **(default applied)**.
4. **Passphrase default** — 5 words + 1 digit, capitalized, `-` separator (~70 bits)
   **(default applied)**. 1Password defaults to 3 words; we chose stronger.
5. **Random password default** — 20 chars, all classes (~130 bits) **(default applied)**.
6. **Argon2id parameters** — desktop 256 MiB / 3 passes / 4 lanes, mobile 128 MiB / 4 / 4,
   hard floor 64 MiB / 3 **(default applied)**. Unlock takes ~0.5–1 s on a modern laptop.
7. **Recovery phrase (24 words)** — opt-in or on by default at setup? See
   `docs/ideas/secure-storage-and-unlock.md` §5.
8. **Master password minimum** — 10 characters for now **(default applied)**; a zxcvbn strength
   check is planned. Stricter rule wanted?
9. **Where the CLI keeps the Secret Key** — a separate owner-only file in the platform config dir
   (`~/Library/Application Support/app.Password365.Password365/secret-key` on macOS) **(default applied)**.
   Desktop/mobile apps will use the OS keychain.
10. **Minimum Rust version** raised to 1.87 (ctutils fix), then to **1.88** (2026-11-09, `time` RUSTSEC-2026-0009 via zxcvbn)
    **(default applied)**.
11. **CSV formula injection** — exported CSV values starting with `=`, `+`, `-`, `@` are written
    unchanged (escaping would corrupt passwords when Chrome imports them). The export warning tells
    users not to open the file in a spreadsheet. OK?
12. **App icon** — placeholder keyhole icon generated (`apps/desktop/app-icon.png`). Replace with a
    real design whenever you like (`npx tauri icon <file>` regenerates all sizes).
13. **Bundle identifier** — `app.password365.desktop` **(default applied)**; depends on the domain you pick.
14. **Extension manual test** — please follow `extension/README.md` (build, `npm run install-host`,
    load `extension/dist` unpacked) and try Fill + Save on a real site. I did not write into your
    browser profiles without asking.
15. ~~**Inline field icon / keyboard shortcut**~~ — **Resolved (you, 2026-11-09): the UX is an
    identical copy of 1Password.** Done: icon inside login fields + menu under the focused field
    (logins for the site, "Use suggested password" on sign-up forms). Keyboard navigation (↓ from
    the field), filtering while typing and the fill shortcut are done too. The shortcut is ⌘⇧L
    (Bitwarden's) because Chrome does not allow 1Password's ⌘\ for extensions.
16. ~~**Apple Developer ID**~~ — **Resolved: you already have the Apple Developer Program.**
    Done: dev builds are signed with your Apple Development certificate by a Cargo runner
    (`apps/desktop/src-tauri/scripts/macos-dev-sign.sh`). Still to do at release: Developer ID
    Application signing + notarization in CI. Original note: **Apple Developer ID (US$ 99/year) for signed + notarized macOS builds** — needed so the
    Keychain never shows the "wants to use your confidential information… enter the login keychain
    password" dialog (unsigned/ad-hoc builds get a new identity on every build, so macOS asks again)
    and so Gatekeeper doesn't warn "unidentified developer". You flagged the dialog as looking
    unsafe (2026-11-09) — agreed: users must never be trained to type their Mac password into
    surprise dialogs. Proposed: dev builds keep the Secret Key in an owner-only file (no dialog);
    release builds are Developer ID-signed and use the Keychain silently, like 1Password.
    Same question later for Windows code signing (Authenticode / Azure Trusted Signing).
17. **Master password strength threshold** — set to zxcvbn score ≥ 3 ("good") plus ≥ 10
    characters (2026-11-09). Rejects `1234567890`, `password123`, `qwertyuiop`, `password3652026`;
    accepts a 4-word passphrase. Lower to 2 ("fair") if it feels too strict — the Secret Key already
    makes offline guessing infeasible, so this mostly protects against someone at your keyboard.

## 18. Sync: state-based merge instead of a signed change log (decided, reversible)
PLAN.md §4.2 described signed per-change records + version vectors. I implemented item states merged
per field (CRDT) — same guarantees (proven by property tests), simpler storage, nothing to replay.
Authenticity moves to the transport/bundle layer (Noise + Ed25519-signed bundles). See
`docs/sync-protocol.md` §1. Say if you'd rather keep the op log (e.g. for an audit trail per change).

## 19. Touch ID / Windows Hello and macOS AutoFill need signed release builds (blocked on you)
Both need the app signed with your own identities:
- **Touch ID** keeps the unlock key in the Keychain behind biometry (`kSecAccessControlBiometryCurrentSet`),
  which only works for a Developer ID–signed, notarized app with the keychain entitlement.
- **macOS AutoFill** (filling in other apps and Safari's password panel) is an
  `AuthenticationServices` credential-provider extension inside the signed app bundle.
- **Windows Hello** uses `KeyCredentialManager`; it works unsigned, but SmartScreen and Hello prompts
  expect an Authenticode-signed app.
Needed from you: Apple Developer Program membership (Team ID, Developer ID Application certificate,
an app-specific password or API key for notarization) and, for Windows, a code-signing certificate
or Azure Trusted Signing account. Until then these stay off; everything else works without them.

## Later (owner, 2026-11-16: "deixa anotado para o futuro, quando o mais importante estiver pronto")
Polish to do once the important features are done — not before:
- Website: screenshots of the desktop app (made-up data, no real brands), more pages, visual polish.
- App Store: compare the logo with the closest password-manager icons; review the store texts and
  screenshots again right before submission (`store-screenshots.sh` regenerates them).
- Extension: verify the conditional passkey flow (passkey offered in the username field's menu)
  on a real site.
