# Threat model (Phase 1 — core + CLI)

## Assets
Passwords, notes, TOTP secrets and also **metadata** (which sites you use, usernames, vault names).

## Assumptions
- The OS and the device are not compromised **while unlocked** (malware with memory access while the
  vault is open can read secrets — true for every password manager).
- The CSPRNG and the audited RustCrypto implementations are correct.

## Threats and mitigations

| # | Threat | Mitigation | Status |
|---|---|---|---|
| T1 | Store file stolen (backup, cloud folder, lost laptop) | Everything encrypted; metadata too; AUK needs password **and** 128-bit Secret Key stored in a separate file; Argon2id 256 MiB | ✅ |
| T2 | Weak master password | Secret Key makes offline brute force infeasible even with weak passwords; 10-char minimum | ✅ (strength meter: Phase 2) |
| T3 | Tampered header lowers KDF cost | KDF params in AAD + hard floor | ✅ |
| T4 | Ciphertexts swapped/rolled back between items or vaults | AAD binds store, record, vault, item and version | ✅ (cross-device rollback: Phase 4 sync log) |
| T5 | Size analysis of records | Padding to 256-byte buckets | ✅ |
| T6 | Deleted data recoverable from file | `PRAGMA secure_delete` | ✅ (SSD wear-levelling caveat) |
| T7 | Other local users read files | 0600 files / 0700 dirs on Unix | ✅ Unix · ⏳ Windows ACLs |
| T8 | Secrets in memory after use | `zeroize` on keys and secret strings | ✅ partial (see gaps) |
| T9 | Secrets in logs / debug output | Redacted `Debug` for all secret types | ✅ |
| T10 | Plaintext CSV left on disk | Warnings on import/export; export file 0600, refuses overwrite | ✅ (secure delete helper: later) |
| T11 | Shoulder surfing | Secrets masked unless `--reveal` | ✅ |
| T13 | Desktop: store + Secret Key in the same folder on macOS → one backup contains both | Secret Key lives in the OS keychain (macOS Keychain / Windows Credential Manager / Linux Secret Service); an older key file is migrated and deleted; file only as fallback when no keychain exists | ✅ desktop · CLI still uses a file |
| T14 | Other local users / processes talk to the browser bridge | Unix socket in a 0700 directory + peer-credential check (same uid only); host is a stateless relay; the app re-checks the website match before releasing any credential | ✅ Unix · ⏳ Windows named-pipe ACL |
| T15 | Malware running as the same user uses the bridge | Out of scope for the MVP (it could also read memory); later: pairing code shown in app + extension, signed host binary verification | ⏳ |
| T12 | Store copied off-device and attacked | Device-bound key (Secure Enclave/TPM) | ⏳ idea, see `docs/ideas` |

## Known gaps (tracked)
- CSV parsing and serde decoding create intermediate `String`s that are not zeroized.
- `--password-stdin` passwords pass through the stdin pipe buffer.
- No memory locking (`mlock`) yet.
- Windows: file ACLs not restricted yet.
- No clipboard integration yet (auto-clear will be required when added).
