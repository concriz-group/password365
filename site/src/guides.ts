/** The guide pages: their address, in every language, their group and when their facts were last checked. */
export const guides = [
  { slug: "password-manager-without-cloud", key: "noCloud", group: "info", modified: "2026-11-23" },
  { slug: "import-passwords", key: "importing", group: "info", modified: "2026-11-23" },
  { slug: "how-sync-works", key: "sync", group: "info", modified: "2026-11-23" },
  { slug: "passkeys-and-one-time-codes", key: "passkeys", group: "info", modified: "2026-11-23" },
  { slug: "1password-bitwarden-alternative", key: "alternative", group: "compare", modified: "2026-11-23" },
  { slug: "keepassxc-alternative", key: "keepass", group: "compare", modified: "2026-11-23" },
  { slug: "apple-passwords-alternative", key: "apple", group: "compare", modified: "2026-11-23" },
  { slug: "chrome-password-manager-alternative", key: "chrome", group: "compare", modified: "2026-11-23" },
  { slug: "proton-pass-alternative", key: "proton", group: "compare", modified: "2026-11-23" },
  { slug: "enpass-alternative", key: "enpass", group: "compare", modified: "2026-11-23" },
] as const;

/** The pages every language has (besides the guides). */
export const pages = ["", "download", "security", "support", "privacy", "compare"] as const;
