import { useEffect, useState, type FormEvent } from "react";

import { BrandLockup } from "../components/BrandMark";
import { toApiError } from "../api/client";
import { useClient } from "../api/context";
import { ErrorMessage } from "../components/ErrorMessage";
import { PasswordInput } from "../components/PasswordInput";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";

/** Lock screen: master password (plus Secret Key on a new device). */
export function Unlock({ onUnlocked }: { onUnlocked: () => void }) {
  const client = useClient();
  const t = useT();
  const [password, setPassword] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [askSecretKey, setAskSecretKey] = useState(false);
  // Seconds left before another try (after wrong passwords).
  const [wait, setWait] = useState(0);
  useEffect(() => {
    if (wait <= 0) return;
    const tick = setTimeout(() => setWait((s) => s - 1), 1000);
    return () => clearTimeout(tick);
  }, [wait]);
  const unlock = useAction(async () => {
    try {
      await client.unlock(password, askSecretKey ? secretKey : undefined);
      return true;
    } catch (reason) {
      const error = toApiError(reason);
      // A new device, or one that forgot its Secret Key after too many wrong passwords.
      if (error.code === "secret_key_required" || error.code === "validation") setAskSecretKey(true);
      if (error.code === "too_many_attempts") setWait(Number(/(\d+)/.exec(error.message)?.[1] ?? 1));
      throw error;
    }
  });

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (await unlock.run()) {
      setPassword("");
      onUnlocked();
    }
  }

  return (
    <main className="centered card">
      <BrandLockup layout="stacked" />
      <h1>{t("unlock.title")}</h1>
      <form onSubmit={submit}>
        <PasswordInput
          label={t("unlock.masterPassword")}
          value={password}
          onChange={setPassword}
          autoFocus
          autoComplete="current-password"
        />
        {askSecretKey ? (
          <div className="field">
            <label htmlFor="secret-key">{t("unlock.secretKey")}</label>
            <input
              id="secret-key"
              value={secretKey}
              onChange={(e) => setSecretKey(e.target.value)}
              placeholder="Q1-XXXX-XXXX-…"
              spellCheck={false}
              autoComplete="off"
            />
          </div>
        ) : (
          <button type="button" className="link" onClick={() => setAskSecretKey(true)}>
            {t("unlock.newDevice")}
          </button>
        )}
        {toApiError(unlock.error ?? {}).code === "too_many_attempts" ? (
          wait > 0 && (
            <p role="alert" className="error">
              {t("unlock.wait", { count: wait })}
            </p>
          )
        ) : (
          <ErrorMessage error={unlock.error} />
        )}
        {toApiError(unlock.error ?? {}).code === "wrong_credentials" && (
          <p className="hint">{t("unlock.changedElsewhere")}</p>
        )}
        {toApiError(unlock.error ?? {}).code === "secret_key_required" && (
          <p className="hint">{t("unlock.secretKeyForgotten")}</p>
        )}
        <button type="submit" className="primary" disabled={!password || unlock.busy || wait > 0}>
          {unlock.busy ? t("unlock.unlocking") : t("unlock.unlock")}
        </button>
      </form>
    </main>
  );
}
