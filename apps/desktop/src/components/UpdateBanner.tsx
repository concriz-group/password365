import { useEffect, useState } from "react";

import type { Subscription } from "../App";
import type { UpdateInfo } from "../api/types";
import { useClient } from "../api/context";
import { useSettings } from "../settings";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

/**
 * Updates, above the vault: the first-run question (checking is a network
 * feature, off until answered) and "a new version is available" when an
 * automatic check found one.
 */
export function UpdateBanner({ subscribeUpdate }: { subscribeUpdate?: Subscription<UpdateInfo> }) {
  const t = useT();
  const { settings, loaded, update } = useSettings();
  const [found, setFound] = useState<UpdateInfo | null>(null);
  useEffect(() => subscribeUpdate?.(setFound), [subscribeUpdate]);

  if (loaded && settings.update_checks === "unasked") {
    return (
      <div className="banner" role="note">
        <span>{t("updates.ask")}</span>
        <button type="button" onClick={() => void update({ update_checks: "automatic" })}>
          {t("updates.askYes")}
        </button>
        <button type="button" className="secondary" onClick={() => void update({ update_checks: "manual" })}>
          {t("updates.askNo")}
        </button>
      </div>
    );
  }
  return found ? <InstallUpdate update={found} /> : null;
}

/** "Password365 0.2.0 is available. [Install and restart]" */
export function InstallUpdate({ update }: { update: UpdateInfo }) {
  const t = useT();
  const client = useClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  async function install() {
    setBusy(true);
    setError(null);
    try {
      await client.updateInstall();
    } catch (e) {
      setError(e);
      setBusy(false);
    }
  }
  return (
    <div className="banner" role="status">
      <span>{t("updates.available", { version: update.version })}</span>
      <button type="button" disabled={busy} onClick={() => void install()}>
        {busy ? t("updates.installing") : t("updates.install")}
      </button>
      <ErrorMessage error={error} />
    </div>
  );
}
