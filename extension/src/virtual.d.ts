// Repository data bundled at build time (see scripts/dataModules.mjs).
declare module "virtual:autofill" {
  /** How a form field is recognised: tried in order, countries' tokens first. */
  export const AUTOFILL: { token: string; kind: "identity" | "card"; hints: string[] }[];
}
declare module "virtual:i18n" {
  /** `locales/<tag>/<area>.json` → its content. */
  export const FILES: Record<string, unknown>;
}
