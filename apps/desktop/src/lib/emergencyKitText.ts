import type { KitText } from "../api/client";
import { formatDate } from "./format";

type Translate = (key: string, params?: Record<string, string | number>) => string;

/** The Emergency Kit sheet's words in the interface language (for the PDF kept in the vault). */
export function emergencyKitText(t: Translate, createdAt: number): KitText {
  return {
    title: t("kit.title"),
    created: t("kit.created", { date: formatDate(createdAt) }),
    keep: t("kit.keep"),
    secret_key_label: t("unlock.secretKey"),
    master_password_label: t("unlock.masterPassword"),
    write_here: t("kit.writeHere"),
    how_to: t("kit.howTo"),
    steps: [
      t("kit.step1"),
      t("kit.step2", { join: t("onboarding.join"), restore: t("onboarding.restore") }),
      t("kit.step3"),
    ],
  };
}
