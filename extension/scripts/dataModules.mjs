// Repository data the extension bundles, served as two virtual modules so the
// bundler (esbuild) and the tests (vitest) read the same files:
//   virtual:autofill — how form fields are recognised (templates/autofill.json
//                      plus every country's tokens, countries first)
//   virtual:i18n     — every translation file (i18n/locales/<tag>/<area>.json)
import { readdirSync, readFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "../..");
const json = (path) => JSON.parse(readFileSync(path, "utf8"));
const list = (dir) => (existsSync(dir) ? readdirSync(dir).sort() : []);

export function autofillFields() {
  const regions = list(join(repo, "templates/regions")).filter((f) => f.endsWith(".json"));
  const fromRegions = regions.flatMap((file) =>
    Object.entries(json(join(repo, "templates/regions", file)).autofill ?? {}).map(([token, spec]) => ({ token, ...spec })),
  );
  return [...fromRegions, ...json(join(repo, "templates/autofill.json")).fields];
}

export function translationFiles() {
  const root = join(repo, "i18n/locales");
  const files = {};
  for (const tag of list(root)) {
    for (const file of list(join(root, tag)).filter((f) => f.endsWith(".json"))) {
      files[`locales/${tag}/${file}`] = json(join(root, tag, file));
    }
  }
  return files;
}

/** Module source for a virtual id, or undefined. */
export function virtualModule(id) {
  if (id === "virtual:autofill") return `export const AUTOFILL = ${JSON.stringify(autofillFields())};`;
  if (id === "virtual:i18n") return `export const FILES = ${JSON.stringify(translationFiles())};`;
  return undefined;
}

/** The same modules for esbuild. */
export const esbuildData = {
  name: "repository-data",
  setup(build) {
    build.onResolve({ filter: /^virtual:(autofill|i18n)$/ }, (args) => ({ path: args.path, namespace: "data" }));
    build.onLoad({ filter: /.*/, namespace: "data" }, (args) => ({ contents: virtualModule(args.path), loader: "js" }));
  },
};

/** The same modules for vite / vitest. */
export const viteData = {
  name: "repository-data",
  resolveId: (id) => (virtualModule(id) === undefined ? undefined : `\0${id}`),
  load: (id) => (id.startsWith("\0") ? virtualModule(id.slice(1)) : undefined),
};
