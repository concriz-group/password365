// Static site for password365.app: English at /, other languages under /<tag>/.
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://password365.app",
  trailingSlash: "always",
  build: { format: "directory" },
  vite: { server: { fs: { allow: [".."] } } },
});
