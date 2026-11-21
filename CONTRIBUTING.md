# Contributing to Password365

Thank you for helping! Every kind of contribution counts: bug reports, ideas, translations, code
and reviews.

- **Bugs and ideas:** [open an issue](https://github.com/concriz-group/password365/issues/new/choose)
  (or, in the app, Settings › About › Help and feedback). Never paste passwords, Secret Keys or
  vault files.
- **Security problems:** privately, never in an issue — see [SECURITY.md](SECURITY.md).

## Translations

Every text the apps show lives in [`i18n/locales/<language>/*.json`](i18n/locales), one file per
area of the app; the website's texts are in [`i18n/site/<language>.json`](i18n/site).

**A new language**
1. Copy `i18n/locales/en` to `i18n/locales/<tag>` (a [BCP 47](https://www.rfc-editor.org/info/bcp47)
   tag: `fr`, `de`, `it`, `pt-PT` …).
2. Set `meta.json` to the language's own name (`{ "name": "Français" }`).
3. Translate the values. Keep every key and `{placeholder}` exactly as in English; plurals use
   `_one` / `_other` (and `_few`, `_many` … where your language has them).
4. Open a pull request. Missing texts fall back to English, so a partial translation is welcome
   too.

The apps pick the new folder up by themselves (desktop: Settings › General › Language). Tests check
that no file has keys English does not have and that placeholders match. Details:
[`i18n/README.md`](i18n/README.md).

**Item fields for a country** (documents, address parts, ID numbers) are data files in
[`templates/`](templates) — see [`templates/README.md`](templates/README.md).

## Code

```sh
cd core && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
cd apps/desktop && npm test && npm run typecheck
cd extension && npm test && npm run typecheck
```

House rules:
- **Tests first.** Every change comes with tests; coverage stays at 90 % or more.
- **Small, single-purpose modules**; no duplicated logic; `unsafe` is forbidden in the core.
- **No texts in code:** every string the user sees goes in `i18n/locales/en/*.json`.
- **Nothing leaves the device unless the user turns it on.** No telemetry, ever.
- **Formats are frozen:** the vault, sync and Secret Key formats are specified in `docs/`;
  changing them needs a new format version and a migration.
- Add a line to [`CHANGELOG.md`](CHANGELOG.md) under "Unreleased" when users will notice the
  change.

Keep pull requests small and focused; describe what changes for the user and why.
