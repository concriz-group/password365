# Field templates and countries

What each category asks for, and how forms are recognised, is data — not code.
The desktop app, the core and the browser extension all read these files.

```
templates/
  categories.json   ← fields of every category, for every country
  autofill.json     ← how the extension recognises form boxes (patterns, in order)
  regions/
    BR.json         ← a country's own fields and form words
```

## A field

```json
{"label": "CPF", "section": "Identification", "autofill": "cpf",
 "mask": "000.000.000-00", "validate": "cpf"}
```

| key | meaning |
|---|---|
| `label` | stored name of the value (never translated; changing it orphans saved values) |
| `input` | `text` (default), `multiline`, `date`, `month_year`, `email`, `phone`, `url`, `totp` |
| `concealed` | hidden until revealed |
| `section` | heading the field sits under |
| `name` | translation key of the shown name when it differs from the label (`"street"` → `fields.street`) |
| `autofill` | what forms call it: an [HTML autocomplete name](https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#autofill) or a token defined in `autofill.json` / the country file |
| `mask` | input mask: `0` digit, `A` letter, `*` anything, other characters are separators |
| `validate` | named check shown as a warning: `cpf`, `cnpj` (BR), `nif_pt` (PT), `dni_es` (ES), `cuit` (AR), `luhn` (cards), `iban` |
| `options` | values to pick from |

Shown names come from `i18n/locales/<language>/fields.json` (keyed by `name`
or `label`) and section headings from `sections.json`.

## Adding a country

1. Create `regions/<ISO 3166 code>.json` (for example `regions/AR.json`).
2. Under `categories`, list the full field list of each category the country
   changes (usually `identity`; also `driver_license`, `passport` …). Reuse the
   shared labels (`first name`, `city`, `zip` …) for the same things, so items
   keep their values when someone switches country.
3. Under `autofill`, add tokens for documents forms ask for, with the words
   (regular expressions) that name them on that country's sites:
   `"dni": {"kind": "identity", "hints": ["\\bdni\\b", "documento"]}`.
4. Add translations of new field names to `i18n/locales/*/fields.json`.

The country then appears in the item editor's "Country or region" list; new
identities follow the country of the app's language. The tests check every
file (unknown categories, duplicate labels, split sections).

## Equivalent domains

`equivalent-domains.json` lists sites that are one account under several domains (`amazon.com`,
`amazon.es`, …): a login saved for one is offered on the others. Only domains the company really
owns go in it. A person can also write a wildcard ending in a login's website (`amazon.*`): that one
login then matches that exact name under any domain ending (never look-alikes such as
`amazon-login.com`).

