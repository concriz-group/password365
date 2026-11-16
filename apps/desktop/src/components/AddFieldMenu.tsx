import { useState } from "react";

import { useT } from "../i18n";
import { FIELD_TYPES, type FieldType } from "../lib/fieldSections";

interface Props {
  /** The section the field goes to, as shown (named in the button for screen readers). */
  section: string;
  onAdd: (type: FieldType) => void;
  disabled?: boolean;
}

/** "+ add another field" and its menu of field types, as in 1Password. */
export function AddFieldMenu({ section, onAdd, disabled }: Props) {
  const t = useT();
  const [open, setOpen] = useState(false);
  return (
    <div className="add-field" onKeyDown={(e) => e.key === "Escape" && setOpen(false)}>
      <button
        type="button"
        className="add-field-button"
        aria-expanded={open}
        aria-label={section ? t("fieldsEditor.addFieldTo", { section }) : t("fieldsEditor.addFieldAria")}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        {t("fieldsEditor.addField")}
      </button>
      {open && (
        <div className="add-field-menu" role="menu">
          {FIELD_TYPES.map((type) => (
            <button
              type="button"
              role="menuitem"
              key={type.key}
              onClick={() => {
                setOpen(false);
                onAdd(type);
              }}
            >
              {t(`fieldTypes.${type.key}`)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
