/** The guide pages: their address, in every language, and when their facts were last checked. */
export const guides = [
  { slug: "password-manager-without-cloud", key: "noCloud", modified: "2026-11-23" },
  { slug: "1password-bitwarden-alternative", key: "alternative", modified: "2026-11-23" },
  { slug: "import-passwords", key: "importing", modified: "2026-11-23" },
  { slug: "how-sync-works", key: "sync", modified: "2026-11-23" },
] as const;

/** The pages every language has (besides the guides). */
export const pages = ["", "download", "security", "support", "privacy"] as const;
