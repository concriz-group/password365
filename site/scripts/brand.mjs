// Copies the brand files (../branding) into public/brand before dev and build,
// so the site always shows the same logo and icon as the apps.
import { cpSync, mkdirSync } from "node:fs";
const out = new URL("../public/brand/", import.meta.url);
mkdirSync(out, { recursive: true });
for (const file of ["password365-logo.svg", "password365-logo-dark.svg", "app-icon.svg", "app-icon-1024.png"]) {
  cpSync(new URL(`../../branding/${file}`, import.meta.url), new URL(file, out));
}
