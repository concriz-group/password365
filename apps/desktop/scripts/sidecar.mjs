// Builds password365-host — the browser extension's helper, shipped inside the app — and puts it
// where Tauri's bundle.externalBin expects it: src-tauri/binaries/password365-host-<target triple>.
// Runs before `tauri dev` (debug build) and `tauri build` (release; a universal macOS build joins
// both architectures with lipo). The app registers the helper with the browsers when it starts.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const core = resolve(here, "../../../core");
const out = resolve(here, "../src-tauri/binaries");
const hostTriple = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (\S+)/m)[1];
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple;
const debug = process.env.TAURI_ENV_DEBUG === "true";
const profile = debug ? "debug" : "release";
const exe = triple.includes("windows") ? ".exe" : "";

/** Builds the helper for one Rust target; returns the binary's path. */
function build(target) {
  const args = ["build", "-q", "-p", "password365-host", "--manifest-path", join(core, "Cargo.toml")];
  if (!debug) args.push("--release");
  // The machine's own target keeps using <target dir>/<profile> (shared with `cargo test`).
  if (target !== hostTriple) args.push("--target", target);
  execFileSync("cargo", args, { stdio: "inherit" });
  const base = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : join(core, "target");
  const dir = target === hostTriple ? join(base, profile) : join(base, target, profile);
  return join(dir, `password365-host${exe}`);
}

mkdirSync(out, { recursive: true });
const dest = join(out, `password365-host-${triple}${exe}`);
if (triple === "universal-apple-darwin") {
  const parts = ["aarch64-apple-darwin", "x86_64-apple-darwin"].map(build);
  execFileSync("lipo", ["-create", "-output", dest, ...parts], { stdio: "inherit" });
} else {
  copyFileSync(build(triple), dest);
}
console.log(`browser helper: ${dest}`);
