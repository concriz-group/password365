// Translation lookup shared by the desktop app and the browser extension.
//
// Catalogs are JSON files under `locales/<language tag>/<area>.json`; keys are
// `<area>.<key>`. A language is added by adding a folder (with `meta.json`
// naming it) — no code changes. Plurals follow the `key_one` / `key_other`
// convention (any CLDR category: `_zero`, `_two`, `_few`, `_many`), picked
// with `Intl.PluralRules` when the call passes a numeric `count`.

/** Flat catalog: `"settings.title" → "Settings"`. */
export type Catalog = Record<string, string>;

/** A language that has a catalog. */
export interface Language {
  tag: string;
  /** Its own name for itself ("Português (Brasil)"). */
  name: string;
}

export type Vars = Record<string, string | number>;
export type Translate = (key: string, vars?: Vars) => string;

/** The language every other one falls back to. */
export const SOURCE_LANGUAGE = "en";

/**
 * Builds catalogs from module paths such as `…/locales/pt-BR/settings.json`
 * (what a bundler's glob import returns).
 */
export function catalogsFromFiles(files: Record<string, unknown>): {
  catalogs: Record<string, Catalog>;
  languages: Language[];
} {
  const catalogs: Record<string, Catalog> = {};
  const names: Record<string, string> = {};
  for (const [path, content] of Object.entries(files)) {
    const match = /locales\/([^/]+)\/([^/]+)\.json$/.exec(path);
    if (!match) continue;
    const [, tag, area] = match as unknown as [string, string, string];
    const data = content as Record<string, unknown>;
    if (area === "meta") {
      names[tag] = String(data.name ?? tag);
      continue;
    }
    const catalog = (catalogs[tag] ??= {});
    for (const [key, value] of Object.entries(data)) {
      if (typeof value === "string") catalog[`${area}.${key}`] = value;
    }
  }
  const languages = Object.keys(catalogs)
    .map((tag) => ({ tag, name: names[tag] ?? tag }))
    .sort((a, b) => a.name.localeCompare(b.name));
  return { catalogs, languages };
}

/**
 * The best available language for `wanted` (most preferred first): an exact
 * tag, then the same base language ("pt-PT" → "pt-BR"), else the source.
 */
export function pickLanguage(wanted: readonly string[], available: readonly string[]): string {
  const lower = available.map((a) => a.toLowerCase());
  for (const tag of wanted) {
    const exact = lower.indexOf(tag.toLowerCase());
    if (exact >= 0) return available[exact]!;
    const base = tag.split("-")[0]!.toLowerCase();
    const sameBase = lower.findIndex((a) => a === base || a.startsWith(`${base}-`));
    if (sameBase >= 0) return available[sameBase]!;
  }
  return SOURCE_LANGUAGE;
}

/** `{name}` placeholders in `text`. */
export function placeholders(text: string): string[] {
  return [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]!).sort();
}

/** A translator for `tag`, falling back to the source language, then to the key itself. */
export function translator(catalogs: Record<string, Catalog>, tag: string): Translate {
  const chain = [catalogs[tag], catalogs[SOURCE_LANGUAGE]].filter((c): c is Catalog => !!c);
  const rules = new Intl.PluralRules(tag);
  const lookup = (key: string) => chain.map((c) => c[key]).find((v) => v !== undefined);
  return (key, vars) => {
    const count = vars?.count;
    const text =
      (typeof count === "number" ? (lookup(`${key}_${rules.select(count)}`) ?? lookup(`${key}_other`)) : undefined) ??
      lookup(key) ??
      key;
    return vars ? text.replace(/\{(\w+)\}/g, (all, name: string) => (name in vars ? String(vars[name]) : all)) : text;
  };
}

/** `t(key)`, or `fallback` when no catalog has `key` (user-made labels, new categories). */
export function translateOr(t: Translate, key: string, fallback: string, vars?: Vars): string {
  const text = t(key, vars);
  return text === key ? fallback : text;
}
