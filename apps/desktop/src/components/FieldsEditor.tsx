import type { ReactNode } from "react";

import type { FieldInput } from "../api/types";
import { useT } from "../i18n";
import { type FieldRow, type FieldType, type Section, grouped } from "../lib/fieldSections";
import { translateOr } from "../../../../i18n/translate";
import { applyMask } from "../lib/mask";
import { isValid } from "../lib/validators";
import { AddFieldMenu } from "./AddFieldMenu";
import { PasswordInput } from "./PasswordInput";

export type { FieldRow } from "../lib/fieldSections";

const HTML_TYPE: Record<Exclude<FieldInput, "multiline">, string> = {
  text: "text",
  date: "date",
  month_year: "month",
  email: "email",
  phone: "tel",
  url: "url",
  totp: "text",
  sign_in_with: "text",
};

interface Props {
  rows: FieldRow[];
  sections: Section[];
  onChange: (id: string, change: Partial<FieldRow>) => void;
  onAdd: (sectionId: string, type: FieldType) => void;
  onRemove: (id: string) => void;
  /** Logins a "Sign in with" field can point at. */
  logins?: { id: string; title: string }[];
  /** Moves a field before another (drag and drop). */
  onMoveBefore?: (id: string, beforeId: string) => void;
  /** Moves a field one place (the handle's arrow keys). */
  onMoveBy?: (id: string, delta: -1 | 1) => void;
  onRenameSection: (id: string, name: string) => void;
  onAddSection: () => void;
  disabled?: boolean;
}

/**
 * The item's fields in sections, like 1Password's editor: each section ends
 * with "add another field", and "add more" starts a new section.
 */
export function FieldsEditor({ rows, sections, onChange, onAdd, onRemove, onMoveBefore, onMoveBy, onRenameSection, onAddSection, disabled, logins = [] }: Props) {
  const t = useT();
  const byId = new Map(sections.map((s) => [s.id, s]));
  return (
    <>
      {grouped(rows, (r) => r.sectionId, sections.map((s) => s.id)).map(([sectionId, sectionRows]) => {
        const section = byId.get(sectionId);
        const name = section?.name ?? "";
        // A category's section shows translated; one the user named shows as typed.
        const shown = section?.custom ? name : translateOr(t, `sections.${name}`, name);
        return (
          <section className="field-section" key={sectionId} aria-label={shown || t("fieldsEditor.fields")}>
            {section?.custom ? (
              <input
                className="section-name"
                aria-label={t("fieldsEditor.sectionName")}
                value={name}
                placeholder={t("fieldsEditor.sectionName")}
                onChange={(e) => onRenameSection(section.id, e.target.value)}
              />
            ) : (
              name && <h3 className="section-title">{shown}</h3>
            )}
            {sectionRows.map((row) => (
              <MovableField key={row.id} row={row} onMoveBefore={onMoveBefore} onMoveBy={onMoveBy}>
                <FieldEditor row={row} onChange={onChange} onRemove={onRemove} disabled={disabled} logins={logins} />
              </MovableField>
            ))}
            <AddFieldMenu section={shown} onAdd={(type) => onAdd(sectionId, type)} disabled={disabled} />
          </section>
        );
      })}
      <button type="button" className="add-section" onClick={onAddSection} disabled={disabled}>
        {t("fieldsEditor.addMore")}
      </button>
    </>
  );
}

/** A field with a ≡ handle: drag it onto another field, or focus it and use ↑ ↓. */
function MovableField({ row, onMoveBefore, onMoveBy, children }: { row: FieldRow; onMoveBefore?: Props["onMoveBefore"]; onMoveBy?: Props["onMoveBy"]; children: ReactNode }) {
  const t = useT();
  const name = row.custom ? row.label : translateOr(t, `fields.${row.spec?.name ?? row.label}`, row.label);
  if (!onMoveBefore && !onMoveBy) return <>{children}</>;
  return (
    <div
      className="movable-field"
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => {
        e.preventDefault();
        const dragged = e.dataTransfer.getData("text/plain");
        if (dragged) onMoveBefore?.(dragged, row.id);
      }}
    >
      <button
        type="button"
        className="drag-handle"
        draggable
        aria-label={t("fieldsEditor.move", { name: capitalize(name || t("fieldsEditor.field")) })}
        title={t("fieldsEditor.moveHint")}
        onDragStart={(e) => e.dataTransfer.setData("text/plain", row.id)}
        onKeyDown={(e) => {
          if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
          e.preventDefault();
          onMoveBy?.(row.id, e.key === "ArrowUp" ? -1 : 1);
          const handle = e.currentTarget;
          requestAnimationFrame(() => handle.focus());
        }}
      >
        ≡
      </button>
      <div className="movable-body">{children}</div>
    </div>
  );
}

interface RowProps {
  row: FieldRow;
  onChange: Props["onChange"];
  onRemove: Props["onRemove"];
  disabled?: boolean;
  logins?: Props["logins"];
}

function FieldEditor({ row, onChange, onRemove, disabled, logins = [] }: RowProps) {
  const t = useT();
  const id = `field-${row.id}`;
  // A category's field shows translated; one the user added shows as typed.
  const name = row.custom
    ? row.label || t("fieldsEditor.field")
    : translateOr(t, `fields.${row.spec?.name ?? row.label}`, row.label);
  const mask = row.spec?.mask;
  const set = (value: string) => onChange(row.id, { value: mask ? applyMask(mask, value) : value });
  const invalid = !isValid(row.spec?.validate, row.value);
  const options = row.spec?.options ?? [];
  return (
    <div className={row.custom ? "custom-field" : undefined}>
      {row.custom && (
        <div className="field-row custom-field-name">
          <input
            aria-label={t("fieldsEditor.nameOf", { name })}
            value={row.label}
            placeholder={t("fieldsEditor.labelPlaceholder")}
            onChange={(e) => onChange(row.id, { label: e.target.value })}
          />
          <button
            type="button"
            className="ghost"
            aria-label={t("fieldsEditor.removeField", { name })}
            onClick={() => onRemove(row.id)}
          >
            ✕
          </button>
        </div>
      )}
      {row.concealed && row.input !== "multiline" ? (
        <PasswordInput label={capitalize(name)} value={row.value} onChange={set} hideLabel={row.custom} />
      ) : (
        <div className="field">
          <label htmlFor={id} className={row.custom ? "sr-only" : undefined}>
            {capitalize(name)}
          </label>
          {row.input === "sign_in_with" ? (
            <select id={id} value={row.value} disabled={disabled} onChange={(e) => set(e.target.value)}>
              <option value="">{t("fieldsEditor.chooseLogin")}</option>
              {logins.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.title}
                </option>
              ))}
            </select>
          ) : options.length > 0 ? (
            <select id={id} value={row.value} disabled={disabled} onChange={(e) => set(e.target.value)}>
              <option value="">{t("common.none")}</option>
              {[...new Set([...options, row.value].filter(Boolean))].map((o) => (
                <option key={o} value={o}>
                  {o}
                </option>
              ))}
            </select>
          ) : row.input === "multiline" ? (
            <textarea
              id={id}
              rows={3}
              value={row.value}
              disabled={disabled}
              spellCheck={false}
              className={row.concealed ? "concealed" : undefined}
              onChange={(e) => set(e.target.value)}
            />
          ) : (
            <input
              id={id}
              type={HTML_TYPE[row.input]}
              value={row.value}
              disabled={disabled}
              autoComplete="off"
              spellCheck={row.input === "totp" ? false : undefined}
              placeholder={row.input === "totp" ? t("itemEditor.totpPlaceholder") : mask?.replace(/[0A*]/g, "_")}
              inputMode={mask && /^[^A*]*$/.test(mask) ? "numeric" : undefined}
              aria-invalid={invalid || undefined}
              aria-describedby={invalid ? `${id}-invalid` : undefined}
              onChange={(e) => set(e.target.value)}
            />
          )}
          {invalid && (
            <p id={`${id}-invalid`} className="hint warn">
              {t("fieldsEditor.invalid", { name })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

export function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
