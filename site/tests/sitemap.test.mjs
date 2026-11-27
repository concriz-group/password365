// The built sitemap (run after `npm run build`): every page has a real last-modified date.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

const file = new URL("../dist/sitemap.xml", import.meta.url);

test("every sitemap entry has a lastmod date", { skip: !existsSync(file) && "no build yet" }, () => {
  const xml = readFileSync(file, "utf8");
  const entries = xml.match(/<url>.*?<\/url>/g) ?? [];
  assert.ok(entries.length > 0);
  for (const e of entries) assert.match(e, /<lastmod>\d{4}-\d{2}-\d{2}<\/lastmod>/, e.slice(0, 80));
});
