// Bundles the extension into dist/ (load it in chrome://extensions → "Load unpacked").
import { build } from "esbuild";
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { esbuildData, translationFiles } from "./dataModules.mjs";
import { firefoxManifest } from "./firefox.mjs";

rmSync("dist", { recursive: true, force: true });
cpSync("static", "dist", { recursive: true });

const common = { plugins: [esbuildData], bundle: true, target: "chrome120", minify: false, sourcemap: false, legalComments: "none" };
await Promise.all([
  build({ ...common, entryPoints: ["src/background/main.ts"], outfile: "dist/background.js", format: "esm" }),
  // Content scripts and the popup are classic scripts: bundle them as IIFEs.
  build({ ...common, entryPoints: ["src/content/main.ts"], outfile: "dist/content.js", format: "iife" }),
  build({ ...common, entryPoints: ["src/popup/main.ts"], outfile: "dist/popup.js", format: "iife" }),
  build({ ...common, entryPoints: ["src/menu/main.ts"], outfile: "dist/menu.js", format: "iife" }),
  // Passkeys: the page-world script (no extension APIs) and its isolated relay.
  build({ ...common, entryPoints: ["src/passkey/pageMain.ts"], outfile: "dist/passkey-page.js", format: "iife" }),
  build({ ...common, entryPoints: ["src/passkey/relayMain.ts"], outfile: "dist/passkey-relay.js", format: "iife" }),
]);
// The store listing's description in every language (chrome's _locales; "pt-BR" → "pt_BR").
const descriptions = Object.entries(translationFiles()).filter(([path]) => path.endsWith("/extension.json"));
for (const [path, catalog] of descriptions) {
  const tag = path.split("/")[1].replace("-", "_");
  mkdirSync(`dist/_locales/${tag}`, { recursive: true });
  const messages = { description: { message: catalog.description } };
  writeFileSync(`dist/_locales/${tag}/messages.json`, JSON.stringify(messages, null, 2));
}
const manifest = JSON.parse(readFileSync("dist/manifest.json", "utf8"));
writeFileSync(
  "dist/manifest.json",
  JSON.stringify({ ...manifest, default_locale: "en", description: "__MSG_description__" }, null, 2),
);
// Firefox: the same files with its own manifest (dist-firefox/; load it from about:debugging).
rmSync("dist-firefox", { recursive: true, force: true });
cpSync("dist", "dist-firefox", { recursive: true });
writeFileSync("dist-firefox/manifest.json", JSON.stringify(firefoxManifest(JSON.parse(readFileSync("dist/manifest.json", "utf8"))), null, 2));
console.log("built dist/ (Chrome, Edge, Brave) and dist-firefox/");
