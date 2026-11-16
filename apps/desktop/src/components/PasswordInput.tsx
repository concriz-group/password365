import { useId, useState } from "react";

import { useT } from "../i18n";

interface Props {
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoFocus?: boolean;
  autoComplete?: string;
  /** Keeps the label for screen readers only (the field's name is shown elsewhere). */
  hideLabel?: boolean;
}

/** Password field with a show/hide toggle. */
export function PasswordInput({ label, value, onChange, autoFocus, autoComplete = "off", hideLabel }: Props) {
  const id = useId();
  const t = useT();
  const [visible, setVisible] = useState(false);
  return (
    <div className="field">
      <label htmlFor={id} className={hideLabel ? "sr-only" : undefined}>
        {label}
      </label>
      <div className="field-row">
        <input
          id={id}
          type={visible ? "text" : "password"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          autoFocus={autoFocus}
          autoComplete={autoComplete}
          spellCheck={false}
        />
        <button
          type="button"
          className="ghost"
          aria-label={t(visible ? "secret.hideLabel" : "secret.showLabel", { label })}
          onClick={() => setVisible((v) => !v)}
        >
          {t(visible ? "common.hide" : "common.show")}
        </button>
      </div>
    </div>
  );
}
