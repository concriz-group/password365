import type { PasswordStrength } from "../api/types";
import { useT } from "../i18n";
import type { Translate } from "../../../../i18n/translate";

/** What to tell the user: the first thing still missing, or that it's fine. */
function advice(strength: PasswordStrength, t: Translate): string {
  if (strength.missing_chars > 0) return t("strength.moreChars", { count: strength.missing_chars });
  if (strength.acceptable) return t("strength.enough");
  return strength.warning || strength.suggestions[0] || t("strength.tooEasy");
}

/** 1Password-style strength bar under a new master password field. */
export function StrengthMeter({ strength }: { strength: PasswordStrength | null }) {
  const t = useT();
  const score = strength?.score ?? 0;
  const tone = strength ? (strength.acceptable ? "ok" : score >= 2 ? "fair" : "weak") : "empty";
  return (
    <div className={`strength strength-${tone}`} aria-live="polite">
      <div
        className="strength-bar"
        role="meter"
        aria-label={t("strength.meter")}
        aria-valuemin={0}
        aria-valuemax={4}
        aria-valuenow={score}
        aria-valuetext={strength ? t(`strength.${strength.label}`) : t("strength.notEntered")}
      >
        {[0, 1, 2, 3, 4].map((i) => (
          <span key={i} className={strength && i <= score ? "filled" : ""} />
        ))}
      </div>
      <p className="strength-text">
        {strength ? (
          <>
            <strong>{t(`strength.${strength.label}`)}</strong> · {advice(strength, t)}
          </>
        ) : (
          t("strength.hint")
        )}
      </p>
    </div>
  );
}
