/**
 * The site's texts, from i18n/site/<language>.json (global by config: a new
 * language is a new file there plus its entry in `languages`).
 */
const files = import.meta.glob("../../i18n/site/*.json", { eager: true, import: "default" }) as Record<string, Catalog>;

type Catalog = { [key: string]: string | Catalog };

/** Language tag → URL prefix ("" for the default, English). */
export const languages = [
  { tag: "en", path: "" },
  { tag: "pt-BR", path: "pt-br" },
  { tag: "es", path: "es" },
] as const;
export type Tag = (typeof languages)[number]["tag"];

export function catalog(tag: string): Catalog {
  const found = Object.entries(files).find(([file]) => file.endsWith(`/${tag}.json`));
  if (!found) throw new Error(`no texts for ${tag}`);
  return found[1];
}

function lookup(c: Catalog, key: string): string | undefined {
  let node: string | Catalog | undefined = c;
  for (const part of key.split(".")) node = typeof node === "object" ? node[part] : undefined;
  return typeof node === "string" ? node : undefined;
}

/** A translator for one language: missing keys fall back to English, then to the key. */
export function translator(tag: string) {
  const own = catalog(tag);
  const english = catalog("en");
  return (key: string, values: Record<string, string | number> = {}) => {
    let text = lookup(own, key) ?? lookup(english, key) ?? key;
    for (const [name, value] of Object.entries(values)) text = text.replaceAll(`{${name}}`, String(value));
    return text;
  };
}

/** The URL of `page` ("" for home, "security", …) in a language. */
export function href(tag: string, page = ""): string {
  const prefix = languages.find((l) => l.tag === tag)?.path ?? "";
  return "/" + [prefix, page].filter(Boolean).join("/") + (prefix || page ? "/" : "");
}

/** getStaticPaths for a page in every language. */
export function languagePaths() {
  return languages.map((l) => ({ params: { lang: l.path || undefined }, props: { tag: l.tag } }));
}
