import { formatDate } from "../lib/format";
import { useT } from "../i18n";

interface Props {
  secretKey: string;
  createdAt: number;
}

/**
 * Printable recovery sheet. Hidden on screen (`print-only`); when printing,
 * everything else is hidden. The master password is never printed — there is
 * a blank box to write it by hand.
 */
export function EmergencyKit({ secretKey, createdAt }: Props) {
  const t = useT();
  return (
    <section className="print-only emergency-kit" aria-label={t("kit.label")}>
      <h1>{t("kit.title")}</h1>
      <p>{t("kit.created", { date: formatDate(createdAt) })}</p>
      <p>{t("kit.keep")}</p>
      <h2>{t("unlock.secretKey")}</h2>
      <p className="kit-secret-key">{secretKey}</p>
      <h2>{t("unlock.masterPassword")}</h2>
      <div className="kit-blank" aria-label={t("kit.writeHere")} />
      <h2>{t("kit.howTo")}</h2>
      <ol>
        <li>{t("kit.step1")}</li>
        <li>{t("kit.step2", { join: t("onboarding.join"), restore: t("onboarding.restore") })}</li>
        <li>{t("kit.step3")}</li>
      </ol>
    </section>
  );
}
