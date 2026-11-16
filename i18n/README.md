# Translations

Every text the desktop app and the browser extension show lives here, one
folder per language (a [BCP 47](https://www.rfc-editor.org/info/bcp47) tag),
one JSON file per area of the app:

```
locales/
  en/          ← source language: every key exists here
    meta.json  ← { "name": "English" } (the language's own name for itself)
    common.json
    settings.json
    …
  pt-BR/      ← the apps (the website uses plain "pt": i18n/site/pt.json)
  es/
```

## Adding a language

1. Copy `locales/en` to `locales/<tag>` (for example `locales/fr` or `locales/de-CH`).
2. Set `meta.json` to the language's own name (`{ "name": "Français" }`).
3. Translate the values. Keep the keys and any `{placeholder}` exactly as in English.

That is all: the app lists every folder in Settings › Language. Missing keys
fall back to English, so a partial translation still works. The tests check
that no file has keys English does not have and that placeholders match.

## Plurals

A text with a number has one key per plural form of the language, as
[CLDR](https://www.unicode.org/cldr/charts/latest/supplemental/language_plural_rules.html)
names them: `items_one`, `items_other` (and `_zero`, `_two`, `_few`, `_many`
where the language has them):

```json
{ "items_one": "{count} item", "items_other": "{count} items" }
```

## The website

The texts of password365.app live apart from the apps' catalogs, in `site/<tag>.json` (one file per
language, nested keys), so the apps do not carry them. A new language is a new file there plus
one line in `site/src/i18n.ts` (`languages`, with its URL prefix). `site/tests` checks that every
file has English's keys and placeholders.
