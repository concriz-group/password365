import { useEffect, useState } from "react";

import type { Settings, SshKeys } from "../api/types";
import { useClient } from "../api/context";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  settings: Settings;
  disabled: boolean;
  set: (change: Partial<Settings>) => void;
}

/** Settings › Developer: the SSH agent and the keys it can offer. */
export function DeveloperSettings({ settings, disabled, set }: Props) {
  const client = useClient();
  const t = useT();
  const [ssh, setSsh] = useState<SshKeys | null>(null);
  const [error, setError] = useState<unknown>(null);

  useEffect(() => {
    client.sshKeys().then(setSsh, setError);
  }, [client]);

  const windows = ssh?.address.startsWith("\\\\") ?? false;
  const config = ssh ? `Host *\n  IdentityAgent ${windows ? ssh.address : `"${ssh.address}"`}` : "";
  return (
    <>
      <h2>{t("settings.section_developer")}</h2>
      <h3>{t("developer.sshAgent")}</h3>
      <div className="field">
        <label className="checkbox" htmlFor="ssh-agent">
          <input id="ssh-agent" type="checkbox" checked={settings.ssh_agent} disabled={disabled} onChange={(e) => set({ ssh_agent: e.target.checked })} />
          {t("developer.useAgent")}
        </label>
        <p className="hint">{t("developer.agentHint")}</p>
      </div>
      {settings.ssh_agent && ssh && (
        <>
          <p className="hint">{t("developer.configHint")}</p>
          <pre className="code-block" aria-label={t("developer.config")}>
            {config}
          </pre>
          <button type="button" className="ghost" onClick={() => void navigator.clipboard?.writeText(config)}>
            {t("common.copy")}
          </button>
        </>
      )}
      <h3>{t("developer.keys")}</h3>
      {ssh && ssh.keys.length === 0 && <p className="hint">{t("developer.noKeys")}</p>}
      <ul className="ssh-keys">
        {ssh?.keys.map((k) => (
          <li key={k.id}>
            <strong>{k.title}</strong>
            {k.fingerprint ? (
              <code className="muted">{k.fingerprint}</code>
            ) : (
              <span className="hint warn">{t(`developer.problem_${k.problem ?? "unreadable"}`)}</span>
            )}
          </li>
        ))}
      </ul>
      <ErrorMessage error={error} />
    </>
  );
}
