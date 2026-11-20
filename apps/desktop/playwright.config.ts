import { defineConfig } from "@playwright/test";

// Browser end-to-end tests of the desktop UI against the in-memory demo backend (`vite dev`, no
// Tauri). The real Rust backend is covered by its own tests; the Tauri WebDriver test lives in
// e2e-tauri/.
export default defineConfig({
  testDir: "e2e",
  timeout: 30_000,
  fullyParallel: true,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  // Locally the Chrome already installed is used (no download); CI installs Playwright's Chromium.
  use: { baseURL: "http://localhost:1420", trace: "retain-on-failure", ...(process.env.CI ? {} : { channel: "chrome" }) },
  webServer: { command: "npm run dev", url: "http://localhost:1420", reuseExistingServer: !process.env.CI, timeout: 60_000 },
});
