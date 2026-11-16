/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // Translations and country templates live in the repository, shared with the extension and the core.
  server: { port: 1420, strictPort: true, fs: { allow: [".", "../../i18n", "../../templates", "../../icons", "../../branding"] } },
  build: { target: "es2022", sourcemap: false },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}", "../../i18n/*.ts"],
      exclude: ["src/main.tsx", "src/dev/**", "src/test/**", "src/**/*.test.{ts,tsx}"],
      thresholds: { lines: 90, functions: 90, statements: 90, branches: 85 },
    },
  },
});
