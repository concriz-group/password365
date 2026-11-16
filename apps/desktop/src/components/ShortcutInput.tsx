import { useState } from "react";

import { useT } from "../i18n";
import { displayShortcut, fromKeyEvent } from "../lib/shortcut";

const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

interface Props {
  id: string;
  label: string;
  value: string;
  disabled?: boolean;
  onChange: (shortcut: string) => void;
  mac?: boolean;
}

/** Records a global shortcut: click, press the keys; × clears it. */
export function ShortcutInput({ id, label, value, disabled, onChange, mac = isMac }: Props) {
  const t = useT();
  const [recording, setRecording] = useState(false);
  return (
    <div className="field shortcut">
      <label htmlFor={id}>{label}</label>
      <div className="field-row">
        <input
          id={id}
          readOnly
          disabled={disabled}
          value={recording ? "" : displayShortcut(value, mac)}
          placeholder={recording ? t("shortcuts.press") : t("shortcuts.none")}
          onFocus={() => setRecording(true)}
          onBlur={() => setRecording(false)}
          onKeyDown={(e) => {
            if (e.key === "Tab") return;
            e.preventDefault();
            if (e.key === "Escape") {
              e.currentTarget.blur();
              return;
            }
            const shortcut = fromKeyEvent(e.nativeEvent);
            if (shortcut) {
              onChange(shortcut);
              e.currentTarget.blur();
            }
          }}
        />
        <button type="button" className="ghost" aria-label={t("shortcuts.clear", { label })} disabled={disabled || !value} onClick={() => onChange("")}>
          ×
        </button>
      </div>
    </div>
  );
}
