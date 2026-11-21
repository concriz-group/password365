import { useState, type FormEvent } from "react";

import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

/**
 * Erases this computer's vault after the person types the confirmation word. Works while locked
 * (a forgotten master password must not trap the vault), so it never asks for the password.
 */
export function EraseVault({ onErased, onCancel }: { onErased?: () => void; onCancel: () => void }) {
  const client = useClient();
  const t = useT();
  const word = t("erase.word");
  const [typed, setTyped] = useState("");
  const erase = useAction(async () => {
    await client.eraseVault();
    onErased?.();
  });
  const ready = typed.trim().toLowerCase() === word.toLowerCase();

  function submit(e: FormEvent) {
    e.preventDefault();
    if (ready) void erase.run();
  }

  return (
    <form onSubmit={submit} aria-label={t("erase.title")} className="erase">
      <h2>{t("erase.title")}</h2>
      <p>{t("erase.what")}</p>
      <p>{t("erase.lost")}</p>
      <p className="hint">{t("erase.then")}</p>
      <label htmlFor="erase-word">{t("erase.type", { word })}</label>
      <input id="erase-word" value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" spellCheck={false} />
      <ErrorMessage error={erase.error} />
      <div className="actions">
        <button type="button" onClick={onCancel}>
          {t("erase.cancel")}
        </button>
        <button type="submit" className="danger" disabled={!ready || erase.busy}>
          {erase.busy ? t("erase.working") : t("erase.confirm")}
        </button>
      </div>
    </form>
  );
}
