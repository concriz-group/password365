import { createContext, useCallback, useContext, useEffect, useMemo, type ReactNode } from "react";

import { catalogsFromFiles, pickLanguage, translator, type Language, type Translate } from "../../../../i18n/translate";
import { useSettings } from "../settings";

// Every catalog in the repository's i18n/locales folder, bundled at build time.
const files = import.meta.glob("../../../../i18n/locales/*/*.json", { eager: true, import: "default" });
export const { catalogs, languages } = catalogsFromFiles(files);

/** The languages the system prefers, most preferred first. */
export const systemLanguages = (): readonly string[] =>
  typeof navigator === "undefined" ? [] : navigator.languages?.length ? navigator.languages : [navigator.language];

interface I18n {
  t: Translate;
  /** The language in use. */
  language: string;
  /** The saved choice; "" follows the system. */
  preference: string;
  languages: Language[];
  /** Saves and applies a new choice. */
  choose: (preference: string) => Promise<void>;
}

/** English, for code outside React and tests. */
export const englishT: Translate = translator(catalogs, "en");

const resolve = (preference: string) =>
  pickLanguage(preference ? [preference] : systemLanguages(), Object.keys(catalogs));

const english: I18n = {
  t: translator(catalogs, "en"),
  language: "en",
  preference: "",
  languages,
  choose: async () => {},
};

const I18nContext = createContext<I18n>(english);

/** Applies the saved language (or the system's) to everything below. */
export function I18nProvider({ children }: { children: ReactNode }) {
  const { settings, update } = useSettings();
  const preference = settings.language;

  const language = resolve(preference);
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  const choose = useCallback((next: string) => update({ language: next }), [update]);

  const value = useMemo(
    () => ({ t: translator(catalogs, language), language, preference, languages, choose }),
    [language, preference, choose],
  );
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

/** The translate function of the current language. */
export const useT = (): Translate => useContext(I18nContext).t;

/** Language state and the choice (Settings › Language). */
export const useI18n = (): I18n => useContext(I18nContext);
