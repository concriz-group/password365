// The extension's texts, in the browser's language (catalogs from the
// repository's i18n folder, bundled at build time).
import { FILES } from "virtual:i18n";

import { catalogsFromFiles, pickLanguage, translateOr, translator, type Vars } from "../../../i18n/translate";

const { catalogs } = catalogsFromFiles(FILES);

/** The languages the browser prefers, most preferred first. */
const preferred = (): readonly string[] =>
  typeof navigator === "undefined" ? [] : navigator.languages?.length ? navigator.languages : [navigator.language];

/** The language in use. */
export const language = pickLanguage(preferred(), Object.keys(catalogs));

const translate = translator(catalogs, language);

/** A text of the `extension` area. */
export const t = (key: string, vars?: Vars): string => translate(`extension.${key}`, vars);

/** The catalogs (tests, and the manifest's `_locales` at build time). */
export { catalogs };

/** `t(key)`, or `fallback` when no catalog has it. */
export const tOr = (key: string, fallback: string): string => translateOr(translate, `extension.${key}`, fallback);
