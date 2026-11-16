import { useState, type FormEvent } from "react";

import { useClient } from "../api/context";
import { ErrorMessage } from "../components/ErrorMessage";
import { PasswordInput } from "../components/PasswordInput";
import { useAction } from "../hooks/useAction";
import { nativeFilePicker, type FilePicker } from "../lib/files";
import { useT } from "../i18n";

interface Props {
  onDone: () => void;
  onBack: () => void;
  files?: FilePicker;
}

/** New computer: restore a backup with the Secret Key and master password. */
export function Restore({ onDone, onBack, files = nativeFilePicker }: Props) {
  const client = useClient();
  const t = useT();
  const [path, setPath] = useState<string | null>(null);
  const [secretKey, setSecretKey] = useState("");
  const [password, setPassword] = useState("");
  const restore = useAction(async (file: string) => {
    await client.restoreBackup(file, password, secretKey.trim());
    return true;
  });

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (path && (await restore.run(path))) onDone();
  }

  return (
    <main className="centered card">
      <h1>{t("restore.title")}</h1>
      <p>
        {t("restore.intro").split("{file}")[0]}
        <code>.p365backup</code>
        {t("restore.intro").split("{file}")[1]}
      </p>
      <form onSubmit={submit}>
        <div className="field">
          <button type="button" onClick={async () => setPath((await files.openBackup()) ?? path)}>
            {t("restore.choose")}
          </button>
          <p className="hint" aria-label={t("restore.file")}>
            {path ?? t("restore.noFile")}
          </p>
        </div>
        <div className="field">
          <label htmlFor="restore-secret-key">{t("unlock.secretKey")}</label>
          <input
            id="restore-secret-key"
            value={secretKey}
            onChange={(e) => setSecretKey(e.target.value)}
            placeholder="P365-XXXXXX-XXXXX-…"
            autoComplete="off"
            spellCheck={false}
          />
        </div>
        <PasswordInput label={t("unlock.masterPassword")} value={password} onChange={setPassword} />
        <ErrorMessage error={restore.error} />
        <button type="submit" className="primary" disabled={!path || !secretKey.trim() || !password || restore.busy}>
          {restore.busy ? t("restore.restoring") : t("restore.restore")}
        </button>
        <button type="button" className="link" onClick={onBack}>
          {t("common.back")}
        </button>
      </form>
    </main>
  );
}
