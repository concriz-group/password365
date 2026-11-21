# Security policy

Password365 keeps people's most sensitive data, so security reports are our first priority.

## Reporting a vulnerability

**Please do not open a public issue.** Instead:

- Use GitHub's private report: [Report a vulnerability](https://github.com/concriz-group/password365/security/advisories/new)
  (only the maintainers see it), or
- write to **hello@password365.app** with "SECURITY" in the subject.

Include what you found, how to reproduce it and what an attacker could do. We will answer within
**7 days**, keep you informed while we fix it, and credit you when the fix is published (unless
you prefer not to be named). Please give us a reasonable time to ship a fix before disclosing.

Never send us real passwords, Secret Keys or vault files: made-up test data is enough.

## Scope

- The Rust core (`core/`): cryptography, the vault, sync, import and export.
- The desktop app (`apps/desktop/`) and the browser extension (`extension/`).
- The update and release process (`.github/workflows/release.yml`, `scripts/release/`).

How the data is protected is described in [`docs/crypto-spec.md`](docs/crypto-spec.md),
[`docs/sync-protocol.md`](docs/sync-protocol.md) and [`docs/threat-model.md`](docs/threat-model.md).

## Supported versions

Security fixes go into the newest release; the apps offer it through their update check.
Password365 has not had an independent audit yet.
