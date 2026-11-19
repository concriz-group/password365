import { useState, type FormEvent } from "react";

import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";
import { emergencyKitText } from "../lib/emergencyKitText";
import { EmergencyKit } from "./EmergencyKit";
import { ErrorMessage } from "./ErrorMessage";
import { PasswordInput } from "./PasswordInput";

/**
 * Settings › Emergency Kit: print a new copy. The master password is asked
 * again; the Secret Key stays in the window only until the sheet is printed.
 */
export function EmergencyKitSection() {
  const client = useClient();
  const t = useT();
  const [password, setPassword] = useState("");
  const [secretKey, setSecretKey] = useState<string | null>(null);

  const print = useAction(async () => {
    setSecretKey(await client.revealSecretKey(password));
    setPassword("");
    try {
      // Let the sheet render before the print dialog opens.
      await new Promise((resolve) => setTimeout(resolve, 0));
      await client.printPage();
    } finally {
      setSecretKey(null);
    }
  });

  const [kept, setKept] = useState(false);
  const keep = useAction(async () => {
    await client.saveEmergencyKit(password, emergencyKitText(t, Date.now()));
    setPassword("");
    setKept(true);
  });

  function submit(e: FormEvent) {
    e.preventDefault();
    void print.run();
  }

  return (
    <>
      <h2>{t("emergencyKit.title")}</h2>
      <p className="hint">{t("emergencyKit.hint")}</p>
      <form onSubmit={submit} aria-label={t("emergencyKit.form")}>
        <PasswordInput label={t("emergencyKit.masterPassword")} value={password} onChange={setPassword} />
        <ErrorMessage error={print.error ?? keep.error} />
        <button type="submit" disabled={!password || print.busy}>
          {t("emergencyKit.print")}
        </button>
        <button type="button" disabled={!password || keep.busy} onClick={() => void keep.run()}>
          {t("emergencyKit.keepInVault")}
        </button>
        {kept && <p className="hint">{t("emergencyKit.kept")}</p>}
      </form>
      {secretKey && <EmergencyKit secretKey={secretKey} createdAt={Date.now()} />}
    </>
  );
}
