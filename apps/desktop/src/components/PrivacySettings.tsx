import type { Settings } from "../api/types";
import { useT } from "../i18n";

/** Settings › Privacy: the only feature that uses the internet, off unless turned on. */
export function PrivacySettings({ settings, disabled, set }: { settings: Settings; disabled: boolean; set: (change: Partial<Settings>) => void }) {
  const t = useT();
  return (
    <>
      <h2>{t("settings.section_privacy")}</h2>
      <p className="hint">{t("privacy.intro")}</p>
      <h3>{t("privacy.watchtower")}</h3>
      <div className="field">
        <label className="checkbox" htmlFor="breach-check">
          <input
            id="breach-check"
            type="checkbox"
            checked={settings.check_breached_passwords}
            disabled={disabled}
            onChange={(e) => set({ check_breached_passwords: e.target.checked })}
          />
          {t("privacy.breachCheck")}
        </label>
        <p className="hint">{t("privacy.breachHint")}</p>
      </div>
    </>
  );
}
