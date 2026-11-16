import { defineConfig } from "vitest/config";

// @ts-expect-error plain JS build helper
import { viteData } from "./scripts/dataModules.mjs";

export default defineConfig({
  plugins: [viteData],
  test: {
    globals: true,
    environment: "jsdom",
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts"],
      exclude: ["src/**/*.test.ts", "src/test/**", "src/*/main.ts", "src/passkey/*Main.ts"],
      thresholds: { lines: 90, functions: 90, statements: 90, branches: 85 },
    },
  },
});
