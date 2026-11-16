import { useState } from "react";

import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

export const MASK = "••••••••";

interface Props {
  label: string;
  /** Fetches the value from the backend (only on reveal or copy). */
  reveal: () => Promise<string | null>;
  copy: (text: string) => Promise<void>;
  multiline?: boolean;
}

/**
 * A secret field: masked by default. The value is fetched from the backend
 * only when the user reveals or copies it, and never kept after hiding.
 */
export function SecretRow({ label, reveal, copy, multiline }: Props) {
  const t = useT();
  const [value, setValue] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<unknown>(null);

  async function fetchSecret(): Promise<string> {
    return (await reveal()) ?? "";
  }

  async function toggle() {
    try {
      setValue(value === null ? await fetchSecret() : null);
    } catch (e) {
      setError(e);
    }
  }

  async function copyValue() {
    try {
      await copy(value ?? (await fetchSecret()));
      setCopied(true);
    } catch (e) {
      setError(e);
    }
  }

  return (
    <div className="detail-row">
      <span className="detail-label">{label}</span>
      <span className={multiline ? "detail-value secret pre" : "detail-value secret"} aria-label={label}>
        {value ?? MASK}
      </span>
      <span className="detail-actions">
        <button type="button" className="ghost" onClick={toggle}>
          {t(value === null ? "common.reveal" : "common.hide")}
        </button>
        <button type="button" className="ghost" onClick={copyValue}>
          {t(copied ? "common.copied" : "common.copy")}
        </button>
      </span>
      <ErrorMessage error={error} />
    </div>
  );
}
