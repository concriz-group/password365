import type { Translate } from "../../../../i18n/translate";
import { englishT } from "../i18n";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** "just now", "5 minutes ago", "3 hours ago", "2 days ago", or "never". */
export function relativeTime(at: number | null, now: number, t: Translate = englishT): string {
  if (at === null) return t("time.never");
  const ago = Math.max(0, now - at);
  if (ago < MINUTE) return t("time.justNow");
  if (ago < HOUR) return t("time.minutesAgo", { count: Math.floor(ago / MINUTE) });
  if (ago < DAY) return t("time.hoursAgo", { count: Math.floor(ago / HOUR) });
  return t("time.daysAgo", { count: Math.floor(ago / DAY) });
}
