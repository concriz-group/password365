import { useState } from "react";

import type { HelpTopic } from "../api/types";
import { useClient } from "../api/context";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

const TOPICS: HelpTopic[] = ["bug", "idea", "translate", "security", "email"];

/** Settings › About › Help and feedback: issues, translations, private security reports. */
export function HelpSection() {
  const t = useT();
  const client = useClient();
  const [error, setError] = useState<unknown>(null);
  return (
    <section aria-labelledby="help-title">
      <h3 id="help-title">{t("help.title")}</h3>
      <p className="hint">{t("help.intro")}</p>
      <ul className="help-links">
        {TOPICS.map((topic) => (
          <li key={topic}>
            <button type="button" className="link" onClick={() => client.openHelp(topic).catch(setError)}>
              {t(`help.${topic}`)}
            </button>
            {topic === "security" && <p className="hint">{t("help.securityHint")}</p>}
          </li>
        ))}
      </ul>
      <p className="hint">{t("help.never")}</p>
      <ErrorMessage error={error} />
    </section>
  );
}
