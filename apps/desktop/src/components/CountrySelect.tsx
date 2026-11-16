import { useCategories } from "../hooks/useCategories";
import { systemLanguages, useI18n } from "../i18n";
import { systemRegion } from "../lib/fieldSections";

/** "ZZ": the user's country has no fields of its own; use the international ones. */
export const INTERNATIONAL = "ZZ";

interface Props {
  value: string;
  disabled: boolean;
  onChange: (region: string) => void;
}

/**
 * Settings › General › Country or region: which country's fields new
 * identities use (documents, address parts). Independent of the language.
 */
export function CountrySelect({ value, disabled, onChange }: Props) {
  const { t, language } = useI18n();
  const categories = useCategories();
  const countries = [...new Set(categories.flatMap((c) => Object.keys(c.regions ?? {})))];
  const name = (code: string) => {
    try {
      return new Intl.DisplayNames([language], { type: "region" }).of(code) ?? code;
    } catch {
      return code;
    }
  };
  const sorted = countries.sort((a, b) => name(a).localeCompare(name(b), language));
  const detected = systemRegion(systemLanguages());
  return (
    <div className="field">
      <label htmlFor="region">{t("settings.region")}</label>
      <select id="region" value={value} disabled={disabled} onChange={(e) => onChange(e.target.value)}>
        <option value="">{detected ? t("settings.regionAuto", { country: name(detected) }) : t("settings.regionAutoUnknown")}</option>
        {sorted.map((code) => (
          <option key={code} value={code}>
            {name(code)}
          </option>
        ))}
        <option value={INTERNATIONAL}>{t("settings.regionOther")}</option>
      </select>
      <p className="hint">{t("settings.regionHint")}</p>
    </div>
  );
}
