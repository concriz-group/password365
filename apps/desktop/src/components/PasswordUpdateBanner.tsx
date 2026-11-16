import { useEffect, useState, type FormEvent } from "react";

import type { PasswordUpdate } from "../api/types";
import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";
import { PasswordInput } from "./PasswordInput";

interface Props {
  /** Changes when a sync finished, to re-check. */
  refreshKey?: number;
}

/**
 * "Your master password was changed on Mac": asks for the new password so
 * this device uses it too (as 1Password asks on the other devices).
 */
export function PasswordUpdateBanner({ refreshKey = 0 }: Props) {
  const client = useClient();
  const t = useT();
  const [update, setUpdate] = useState<PasswordUpdate | null>(null);
  const [password, setPassword] = useState("");
  const [done, setDone] = useState(false);

  useEffect(() => {
    let alive = true;
    client.pendingPasswordChange().then(
      (pending) => alive && setUpdate(pending ?? null),
      () => alive && setUpdate(null),
    );
    return () => {
      alive = false;
    };
  }, [client, refreshKey]);

  const adopt = useAction(async () => {
    await client.adoptMasterPassword(password);
    setPassword("");
    setUpdate(null);
    setDone(true);
  });

  function submit(e: FormEvent) {
    e.preventDefault();
    void adopt.run();
  }

  if (done) {
    return (
      <div className="banner" role="status">
        <span>{t("passwordUpdate.adopted")}</span>
        <button type="button" onClick={() => setDone(false)}>
          {t("passwordUpdate.ok")}
        </button>
      </div>
    );
  }
  if (!update) return null;
  return (
    <form className="banner" onSubmit={submit} aria-label={t("passwordUpdate.form")}>
      <span>{t("passwordUpdate.changedOn", { device: update.device_name })}</span>
      <PasswordInput label={t("passwordUpdate.newPassword")} value={password} onChange={setPassword} />
      <button type="submit" className="primary inline" disabled={!password || adopt.busy}>
        {t("passwordUpdate.update")}
      </button>
      <ErrorMessage error={adopt.error} />
    </form>
  );
}
