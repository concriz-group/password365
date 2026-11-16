import { useState } from "react";

import type { UpdateInfo } from "../api/types";
import { useClient } from "../api/context";
import { useSettings } from "../settings";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";
import { InstallUpdate } from "./UpdateBanner";

/** Settings › About: automatic update checks, and "Check for updates" now. */
export function UpdateSettings() {
  const t = useT();
  const client = useClient();
  const { settings, loaded, update } = useSettings();
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ found: UpdateInfo | null } | null>(null);
  const [error, setError] = useState<unknown>(null);
  async function check() {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult({ found: await client.updateCheck() });
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="field">
      <label className="checkbox" htmlFor="update-checks">
        <input
          id="update-checks"
          type="checkbox"
          disabled={!loaded}
          checked={settings.update_checks === "automatic"}
          onChange={(e) => void update({ update_checks: e.target.checked ? "automatic" : "manual" })}
        />
        {t("updates.automatic")}
      </label>
      <p className="hint">{t("updates.automaticHint")}</p>
      <button type="button" disabled={busy} onClick={() => void check()}>
        {busy ? t("updates.checking") : t("updates.check")}
      </button>
      {result && !result.found && <p role="status">{t("updates.upToDate")}</p>}
      {result?.found && <InstallUpdate update={result.found} />}
      <ErrorMessage error={error} />
    </div>
  );
}
