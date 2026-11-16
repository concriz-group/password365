import { useState } from "react";

import type { Breached } from "../api/types";
import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";
import { useSettings } from "../settings";
import { ErrorMessage } from "./ErrorMessage";

/**
 * Watchtower › Vulnerable passwords: logins whose password appeared in data
 * breaches. Asks haveibeenpwned.com only when the user turned it on and clicks.
 */
export function BreachCard({ onOpenItem }: { onOpenItem: (id: string) => void }) {
  const client = useClient();
  const t = useT();
  const { settings } = useSettings();
  const [found, setFound] = useState<Breached[] | null>(null);
  const check = useAction(async () => setFound(await client.breachedPasswords()));

  return (
    <div className="panel-card breach">
      <h2>{t("watchtower.breachTitle")}</h2>
      {!settings.check_breached_passwords ? (
        <p className="muted">{t("watchtower.breachOff")}</p>
      ) : (
        <>
          <p className="muted">{t("watchtower.breachText")}</p>
          <button type="button" onClick={() => void check.run()} disabled={check.busy}>
            {check.busy ? t("watchtower.breachChecking") : t("watchtower.breachCheck")}
          </button>
          <ErrorMessage error={check.error} />
          {found && found.length === 0 && <p className="hint ok">{t("watchtower.breachNone")}</p>}
          {found && found.length > 0 && (
            <ul className="finding-items" aria-label={t("watchtower.breachTitle")}>
              {found.map(({ item, count }) => (
                <li key={item.id}>
                  <button type="button" className="row" onClick={() => onOpenItem(item.id)}>
                    <span className="row-title">{item.title}</span>
                    <span className="muted">{t("watchtower.breachSeen", { count })}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </div>
  );
}
