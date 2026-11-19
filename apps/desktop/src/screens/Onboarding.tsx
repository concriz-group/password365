import { useState, type FormEvent } from "react";

import { BrandLockup } from "../components/BrandMark";
import { useClient } from "../api/context";
import { EmergencyKit } from "../components/EmergencyKit";
import { ErrorMessage } from "../components/ErrorMessage";
import { PasswordInput } from "../components/PasswordInput";
import { StrengthMeter } from "../components/StrengthMeter";
import { useAction } from "../hooks/useAction";
import type { FilePicker } from "../lib/files";
import { JoinDevice } from "./JoinDevice";
import { Restore } from "./Restore";
import { usePasswordStrength } from "../hooks/usePasswordStrength";
import { useT } from "../i18n";
import { emergencyKitText } from "../lib/emergencyKitText";

/** First run: choose a master password, then save the Secret Key. */
export function Onboarding({ onDone, files }: { onDone: () => void; files?: FilePicker }) {
  const client = useClient();
  const t = useT();
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [secretKey, setSecretKey] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [createdAt, setCreatedAt] = useState(0);
  const create = useAction(client.createAccount);
  const print = useAction(client.printPage);

  const strength = usePasswordStrength(password);
  const [restoring, setRestoring] = useState(false);
  const [joining, setJoining] = useState(false);
  const mismatch = confirmation.length > 0 && confirmation !== password;

  async function submit(e: FormEvent) {
    e.preventDefault();
    const key = await create.run(password, confirmation);
    if (key) {
      // The Secret Key also goes into the vault as the Emergency Kit PDF (best effort).
      await client.saveEmergencyKit(password, emergencyKitText(t, Date.now())).catch(() => undefined);
      setSecretKey(key);
      setCreatedAt(Date.now());
      setPassword("");
      setConfirmation("");
    }
  }

  if (restoring) return <Restore onDone={onDone} onBack={() => setRestoring(false)} files={files} />;
  if (joining) return <JoinDevice onDone={onDone} onBack={() => setJoining(false)} />;

  if (secretKey) {
    return (
      <main className="centered card">
        <h1>{t("onboarding.saveKeyTitle")}</h1>
        <p>{t("onboarding.saveKeyText")}</p>
        <p className="secret-key" aria-label={t("unlock.secretKey")}>
          {secretKey}
        </p>
        <p className="hint">{t("onboarding.printHint")}</p>
        <button type="button" onClick={() => void print.run()}>
          {t("onboarding.print")}
        </button>
        <ErrorMessage error={print.error} />
        <EmergencyKit secretKey={secretKey} createdAt={createdAt} />
        <label className="checkbox">
          <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
          {t("onboarding.saved")}
        </label>
        <button type="button" className="primary" disabled={!saved} onClick={onDone}>
          {t("onboarding.open")}
        </button>
      </main>
    );
  }

  return (
    <main className="centered card">
      <BrandLockup layout="stacked" />
      <h1>{t("onboarding.welcome")}</h1>
      <p>{t("onboarding.intro")}</p>
      <form onSubmit={submit}>
        <PasswordInput label={t("unlock.masterPassword")} value={password} onChange={setPassword} autoFocus />
        <StrengthMeter strength={strength} />
        <PasswordInput label={t("onboarding.repeat")} value={confirmation} onChange={setConfirmation} />
        {mismatch && <p className="hint warn">{t("onboarding.mismatch")}</p>}
        <ErrorMessage error={create.error} />
        <button type="submit" className="primary" disabled={!strength?.acceptable || mismatch || !confirmation || create.busy}>
          {create.busy ? t("onboarding.creating") : t("onboarding.create")}
        </button>
      </form>
      <p className="hint">
        {t("onboarding.already")}{" "}
        <button type="button" className="link" onClick={() => setJoining(true)}>
          {t("onboarding.join")}
        </button>{" "}
        {t("onboarding.or")}{" "}
        <button type="button" className="link" onClick={() => setRestoring(true)}>
          {t("onboarding.restore")}
        </button>
      </p>
    </main>
  );
}
