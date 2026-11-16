// The site's texts (i18n/site/*.json): every language has English's keys and placeholders,
// and every key the pages ask for exists.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";

const dir = new URL("../../i18n/site/", import.meta.url);
const load = (file) => JSON.parse(readFileSync(new URL(file, dir), "utf8"));
const flat = (node, prefix = "") =>
  Object.entries(node).flatMap(([k, v]) => (typeof v === "object" ? flat(v, `${prefix}${k}.`) : [[`${prefix}${k}`, v]]));
const english = new Map(flat(load("en.json")));
const placeholders = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

for (const file of readdirSync(dir).filter((f) => f !== "en.json")) {
  test(`${file} has the same keys and placeholders as English`, () => {
    const own = new Map(flat(load(file)));
    assert.deepEqual([...own.keys()].sort(), [...english.keys()].sort());
    for (const [key, text] of own) assert.deepEqual(placeholders(text), placeholders(english.get(key)), key);
  });
}

test("every key the pages use exists", () => {
  const src = new URL("../src/", import.meta.url);
  const files = readdirSync(src, { recursive: true }).filter((f) => f.endsWith(".astro"));
  const used = new Set();
  for (const f of files) {
    const code = readFileSync(new URL(f, src), "utf8");
    for (const m of code.matchAll(/t\("([\w.]+)"/g)) used.add(m[1]);
    // Keys built from parts, e.g. t(`features.${key}.title`), are checked by expanding the lists.
  }
  for (const key of used) assert.ok(english.has(key), `missing ${key}`);
});

test("every templated key resolves for the lists on the home page", () => {
  const groups = {
    "why.{}.title": ["target", "control", "offline"],
    "features.{}.title": ["autofill", "passkeys", "totp", "watchtower", "categories", "generator", "quickAccess", "attachments", "ssh"],
    "security.{}.title": ["keys", "crypto", "local"],
    "sync.{}.title": ["step1", "step2", "step3"],
    "faq.q{}": ["0", "1", "2", "3", "4", "5"],
    "faq.a{}": ["0", "1", "2", "3", "4", "5"],
  };
  for (const [pattern, parts] of Object.entries(groups))
    for (const part of parts) assert.ok(english.has(pattern.replace("{}", part)), pattern.replace("{}", part));
});
