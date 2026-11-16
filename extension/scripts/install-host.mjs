// Registers password365-host as the native-messaging host of the browser
// extension, for the current user, in every installed browser:
//   Chrome, Chrome Beta, Chromium, Brave, Edge (Chromium manifest) and Firefox (its own).
// macOS and Linux: a JSON file in each browser's NativeMessagingHosts folder.
// Windows: the JSON next to the host, and a registry key per browser (HKCU, no admin).
// Usage: npm run install-host [-- /path/to/password365-host]
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir, platform } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { FIREFOX_ID } from "./firefox.mjs";

const NAME = "app.password365.host";
const here = dirname(fileURLToPath(import.meta.url));
const { id } = JSON.parse(readFileSync(join(here, "extension-key.json"), "utf8"));
const exe = platform() === "win32" ? ".exe" : "";
const core = resolve(here, "../../core/target");
const host =
  process.argv[2] ?? [join(core, `release/password365-host${exe}`), join(core, `debug/password365-host${exe}`)].find(existsSync);
if (!host || !existsSync(host)) {
  console.error("password365-host not found. Build it first: (cd core && cargo build --release -p password365-host)");
  process.exit(1);
}

const base = { name: NAME, description: "Password365 browser helper", path: resolve(host), type: "stdio" };
const chromium = { ...base, allowed_origins: [`chrome-extension://${id}/`] };
const firefox = { ...base, allowed_extensions: [FIREFOX_ID] };
const home = homedir();
let installed = 0;

function writeManifest(dir, manifest) {
  mkdirSync(dir, { recursive: true });
  const file = join(dir, `${NAME}.json`);
  writeFileSync(file, JSON.stringify(manifest, null, 2) + "\n");
  return file;
}

if (platform() === "win32") {
  const dir = join(process.env.LOCALAPPDATA ?? home, "Password365", "NativeMessagingHosts");
  const chromiumFile = writeManifest(join(dir, "chromium"), chromium);
  const firefoxFile = writeManifest(join(dir, "firefox"), firefox);
  const keys = [
    ["Chrome", "HKCU\\Software\\Google\\Chrome\\NativeMessagingHosts", chromiumFile],
    ["Edge", "HKCU\\Software\\Microsoft\\Edge\\NativeMessagingHosts", chromiumFile],
    ["Brave", "HKCU\\Software\\BraveSoftware\\Brave-Browser\\NativeMessagingHosts", chromiumFile],
    ["Chromium", "HKCU\\Software\\Chromium\\NativeMessagingHosts", chromiumFile],
    ["Firefox", "HKCU\\Software\\Mozilla\\NativeMessagingHosts", firefoxFile],
  ];
  for (const [browser, key, file] of keys) {
    execFileSync("reg", ["add", `${key}\\${NAME}`, "/ve", "/t", "REG_SZ", "/d", file, "/f"], { stdio: "ignore" });
    console.log(`installed for ${browser}`);
    installed++;
  }
} else {
  const mac = platform() === "darwin";
  const support = mac ? join(home, "Library/Application Support") : join(home, ".config");
  const chromiumBrowsers = mac
    ? ["Google/Chrome", "Google/Chrome Beta", "Chromium", "BraveSoftware/Brave-Browser", "Microsoft Edge"]
    : ["google-chrome", "chromium", "BraveSoftware/Brave-Browser", "microsoft-edge"];
  for (const browser of chromiumBrowsers.map((b) => join(support, b)).filter(existsSync)) {
    writeManifest(join(browser, "NativeMessagingHosts"), chromium);
    console.log(`installed for ${browser}`);
    installed++;
  }
  const mozilla = mac ? join(support, "Mozilla") : join(home, ".mozilla");
  if (existsSync(mozilla) || existsSync(mac ? "/Applications/Firefox.app" : "/usr/bin/firefox")) {
    writeManifest(join(mozilla, mac ? "NativeMessagingHosts" : "native-messaging-hosts"), firefox);
    console.log("installed for Firefox");
    installed++;
  }
}
console.log(installed ? `host: ${base.path}\nextension ids: ${id} (Chromium), ${FIREFOX_ID} (Firefox)` : "No supported browser found.");
