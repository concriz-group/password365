import { useEffect, useState, type FormEvent } from "react";

import type { Item, ItemInput, ItemKind, Vault } from "../api/types";
import { useClient } from "../api/context";
import { useCategories } from "../hooks/useCategories";
import { DEFAULT_SEQUENCE, SEQUENCE_LABEL, WINDOW_LABEL, isAutoTypeField } from "../lib/autoType";
import { DEFAULT_PASSWORD } from "../lib/generatorPresets";
import { splitList } from "../lib/format";
import { useAction } from "../hooks/useAction";
import { ErrorMessage } from "./ErrorMessage";
import {
  type FieldRow,
  type FieldType,
  type Section,
  addField,
  asItemFields,
  defaultRegion,
  duplicateLabel,
  initialFields,
  moveBefore,
  moveBy,
  newId,
  regionsOf,
  templateIn,
  toFieldValues,
} from "../lib/fieldSections";
import { systemLanguages, useI18n, useT } from "../i18n";
import { useSettings } from "../settings";
import { translateOr } from "../../../../i18n/translate";
import { FieldsEditor } from "./FieldsEditor";
import { IconPicker } from "./IconPicker";
import { nativeFilePicker, type FilePicker } from "../lib/files";
import { PasswordInput } from "./PasswordInput";

interface Props {
  /** Existing item to edit, or `null` to create a new one. */
  item: Item | null;
  /** Category of a new item (chosen on the "What would you like to add?" screen). */
  kind?: ItemKind;
  vaults: Vault[];
  defaultVaultId: string | null;
  onSaved: (item: Item) => void;
  onCancel: () => void;
  /** File dialogs (a new Document's file). */
  files?: FilePicker;
}

/** Create / edit form. Empty secret fields on edit mean "keep the current value". */
export function ItemEditor({ item, kind: newKind = "login", vaults, defaultVaultId, onSaved, onCancel, files = nativeFilePicker }: Props) {
  const client = useClient();
  const t = useT();
  const { language } = useI18n();
  const { settings, loaded: settingsLoaded } = useSettings();
  const categories = useCategories();
  const editing = item !== null;
  const kind = item?.kind ?? newKind;
  const category = categories.find((c) => c.kind === kind);
  const [rows, setRows] = useState<FieldRow[] | null>(null);
  const [sections, setSections] = useState<Section[]>([]);
  const [region, setRegion] = useState(item?.region ?? "");
  // Logins a "Sign in with" field can point at (not this item).
  const [logins, setLogins] = useState<{ id: string; title: string }[]>([]);
  useEffect(() => {
    let live = true;
    client.items("", undefined).then(
      (all) => live && setLogins((all ?? []).filter((i) => i.kind === "login" && i.id !== item?.id).map((i) => ({ id: i.id, title: i.title }))),
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [client, item?.id]);
  // undefined: unchanged; null: removed; a data URL: chosen.
  const [icon, setIcon] = useState<string | null | undefined>(undefined);
  const shownIcon = icon === undefined ? (item?.icon ?? null) : icon;
  const [rowsLoaded, setRowsLoaded] = useState(!item?.fields.some((f) => f.concealed || f.input === "totp"));
  const shown = (label: string) => item?.fields.find((f) => f.label.toLowerCase() === label)?.value ?? "";
  const [sequence, setSequence] = useState(shown(SEQUENCE_LABEL));
  const [windows, setWindows] = useState(shown(WINDOW_LABEL));
  const [vaultId, setVaultId] = useState(item?.vault_id ?? defaultVaultId ?? vaults[0]?.id ?? "");
  const [title, setTitle] = useState(item?.title ?? "");
  const [username, setUsername] = useState(item?.username ?? "");
  const [password, setPassword] = useState("");
  const [totp, setTotp] = useState("");
  const [urls, setUrls] = useState(item?.urls.join("\n") ?? "");
  const [tags, setTags] = useState(item?.tags.join(", ") ?? "");
  const [notes, setNotes] = useState("");
  const [notesLoaded, setNotesLoaded] = useState(!item?.has_notes);

  // The rows need the category's template; concealed values are fetched only when editing.
  useEffect(() => {
    if (rows !== null || (categories.length === 0 && kind !== "login") || (!item && !settingsLoaded)) return;
    const own = (item?.fields ?? []).filter((f) => !isAutoTypeField(f.label));
    const startRegion = item ? (item.region ?? "") : defaultRegion(category, settings.region, systemLanguages());
    setRegion(startRegion);
    const first = initialFields(category && { ...category, fields: templateIn(category, startRegion) }, own);
    setRows(first.rows);
    setSections(first.sections);
    const hidden = own.filter((f) => f.concealed || f.input === "totp");
    if (hidden.length === 0) return;
    void Promise.all(hidden.map((f) => client.revealField(item!.id, f.label))).then((values) => {
      setRows((current) =>
        current?.map((r) => {
          const i = hidden.findIndex((f) => f.label === r.label);
          return i < 0 ? r : { ...r, value: values[i] ?? "" };
        }) ?? current,
      );
      setRowsLoaded(true);
    });
    // The country settings only matter for the first layout of a new item.
  }, [client, item, category, categories.length, kind, rows, settingsLoaded]);

  /** Switches to another country's fields, keeping every value typed so far. */
  function changeRegion(next: string) {
    if (!rows || !category) return;
    const rebuilt = initialFields({ ...category, fields: templateIn(category, next) }, asItemFields(rows, sections));
    setRegion(next);
    setRows(rebuilt.rows);
    setSections(rebuilt.sections);
  }

  const countryName = (code: string) => {
    try {
      return new Intl.DisplayNames([language], { type: "region" }).of(code) ?? code;
    } catch {
      return code;
    }
  };

  const editRows = (change: (rows: FieldRow[]) => FieldRow[]) => setRows((current) => (current ? change(current) : current));

  function setField(id: string, change: Partial<FieldRow>) {
    editRows((all) => all.map((r) => (r.id === id ? { ...r, ...change } : r)));
  }

  function addFieldTo(sectionId: string, type: FieldType) {
    editRows((all) => addField(all, sectionId, type, t(`fieldTypes.${type.key}`)));
  }

  function removeField(id: string) {
    editRows((all) => all.filter((r) => r.id !== id));
  }

  function addSection() {
    const taken = new Set(sections.map((s) => s.name));
    const base = t("itemEditor.newSection");
    let name = base;
    for (let n = 2; taken.has(name); n++) name = `${base} ${n}`;
    setSections((all) => [...all, { id: newId("section"), name, custom: true }]);
  }

  function renameSection(id: string, name: string) {
    setSections((all) => all.map((s) => (s.id === id ? { ...s, name } : s)));
  }

  // Notes are a secret: only fetched when an existing item with notes is edited.
  useEffect(() => {
    if (item?.has_notes) {
      void client.reveal(item.id, "notes").then((n) => {
        setNotes(n ?? "");
        setNotesLoaded(true);
      });
    }
  }, [client, item]);

  const save = useAction(async () => {
    const twice = rows && duplicateLabel(rows);
    if (twice) throw new Error(t("itemEditor.duplicate", { label: twice }));
    const input: ItemInput = {
      vault_id: vaultId || undefined,
      kind: editing ? undefined : kind,
      title,
      username,
      urls: splitList(urls),
      tags: splitList(tags),
      favorite: item?.favorite ?? false,
      password: editing && password === "" ? undefined : password,
      totp: editing && totp === "" ? undefined : totp,
      notes: notesLoaded ? notes : undefined,
      fields: formFields(),
      region: rows ? region : undefined,
      icon: icon === undefined ? undefined : (icon ?? ""),
    };
    return editing ? client.updateItem(item.id, item.version, input) : client.createItem(input);
  });

  /** Every custom field, or `undefined` to leave them as they are. */
  function formFields() {
    if (!rows || !rowsLoaded) return undefined;
    const fields = [
      ...toFieldValues(rows, sections),
      { label: SEQUENCE_LABEL, value: sequence.trim() },
      { label: WINDOW_LABEL, value: windows.trim() },
    ].filter((f) => f.value !== "" || rows.some((r) => r.label === f.label));
    return fields.length > 0 || (item?.fields.length ?? 0) > 0 ? fields : undefined;
  }

  const generate = useAction(async () => {
    const generated = await client.generate(DEFAULT_PASSWORD);
    setPassword(generated.value);
  });

  // A new Document: the file chosen before saving, attached right after.
  const [documentPath, setDocumentPath] = useState<string | null>(null);
  const isNewDocument = !editing && kind === "document";
  const attachDocument = useAction(async (saved: Item) => {
    if (documentPath) await client.attachFile(saved.id, documentPath);
  });

  async function submit(e: FormEvent) {
    e.preventDefault();
    const saved = await save.run();
    if (!saved) return;
    if (isNewDocument && documentPath) await attachDocument.run(saved);
    onSaved(saved);
  }

  return (
    <form className="item-editor" onSubmit={submit} aria-label={t(editing ? "itemEditor.ariaEdit" : "itemEditor.ariaNew")}>
      <header className="editor-header">
        <h1>
          {editing
            ? t("itemEditor.editTitle", { title: item.title })
            : category
              ? t("itemEditor.newOf", { category: translateOr(t, `categories.${kind}`, category.label) })
              : t("itemEditor.newItem")}
        </h1>
        <div className="editor-actions">
          <button type="button" className="ghost" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button type="submit" className="primary inline" disabled={!title.trim() || save.busy}>
            {t("common.save")}
          </button>
        </div>
      </header>
      <div className="field">
        <label htmlFor="vault">{t("itemEditor.vault")}</label>
        <select id="vault" value={vaultId} onChange={(e) => setVaultId(e.target.value)}>
          {vaults.map((v) => (
            <option key={v.id} value={v.id}>
              {v.name}
            </option>
          ))}
        </select>
      </div>
      <IconPicker title={title} kind={kind} icon={shownIcon} onChange={setIcon} />
      {isNewDocument && (
        <div className="field">
          <p className="hint">{t("attachments.documentHint")}</p>
          <div className="field-row">
            <button
              type="button"
              onClick={() =>
                void files.openAnyFile().then((path) => {
                  if (!path) return;
                  setDocumentPath(path);
                  if (!title.trim()) setTitle(path.split(/[\\/]/).pop() ?? "");
                })
              }
            >
              {t("attachments.chooseFile")}
            </button>
            {documentPath && <span className="muted">{t("attachments.chosen", { name: documentPath.split(/[\\/]/).pop() ?? "" })}</span>}
          </div>
        </div>
      )}
      <div className="field">
        <label htmlFor="title">{t("itemEditor.title")}</label>
        <input id="title" value={title} onChange={(e) => setTitle(e.target.value)} autoFocus />
      </div>
      {regionsOf(category).length > 0 && (
        <div className="field">
          <label htmlFor="region">{t("itemEditor.region")}</label>
          <select
            id="region"
            value={region}
            disabled={!rows || !rowsLoaded}
            onChange={(e) => changeRegion(e.target.value)}
          >
            <option value="">{t("itemEditor.international")}</option>
            {regionsOf(category).map((code) => (
              <option key={code} value={code}>
                {countryName(code)}
              </option>
            ))}
          </select>
        </div>
      )}
      {(kind === "login" || kind === "password") && (
        <>
          {kind === "login" && (
            <div className="field">
              <label htmlFor="username">{t("itemEditor.username")}</label>
              <input id="username" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="off" />
            </div>
          )}
          <PasswordInput
            label={t(editing && item.has_password ? "itemEditor.newPassword" : "itemEditor.password")}
            value={password}
            onChange={setPassword}
          />
          <button type="button" onClick={() => void generate.run()}>
            {t("itemEditor.generate")}
          </button>
        </>
      )}
      {kind === "login" && (
        <>
          <div className="field">
            <label htmlFor="totp">
              {t(editing && item.has_totp ? "itemEditor.totpKeep" : "itemEditor.totp")}
            </label>
            <input
              id="totp"
              value={totp}
              onChange={(e) => setTotp(e.target.value)}
              placeholder={t("itemEditor.totpPlaceholder")}
              autoComplete="off"
              spellCheck={false}
            />
          </div>
          <div className="field">
            <label htmlFor="urls">{t("itemEditor.websites")}</label>
            <textarea id="urls" rows={2} value={urls} onChange={(e) => setUrls(e.target.value)} />
          </div>
        </>
      )}
      {rows && (
        <FieldsEditor
          rows={rows}
          sections={sections}
          onChange={setField}
          onAdd={addFieldTo}
          onRemove={removeField}
          onMoveBefore={(id, before) => editRows((all) => moveBefore(all, id, before))}
          onMoveBy={(id, delta) => editRows((all) => moveBy(all, id, delta))}
          logins={logins}
          onRenameSection={renameSection}
          onAddSection={addSection}
          disabled={!rowsLoaded}
        />
      )}
      {(kind === "login" || kind === "password") && (
        <details className="auto-type-settings" open={sequence !== "" || windows !== ""}>
          <summary>{t("itemEditor.autoType")}</summary>
          <p className="muted small">{t("itemEditor.autoTypeHelp")}</p>
          <div className="field">
            <label htmlFor="auto-type-sequence">{t("itemEditor.sequence")}</label>
            <input
              id="auto-type-sequence"
              value={sequence}
              placeholder={DEFAULT_SEQUENCE}
              spellCheck={false}
              autoComplete="off"
              onChange={(e) => setSequence(e.target.value)}
            />
            <p className="muted small">{t("itemEditor.sequenceHelp")}</p>
          </div>
          <div className="field">
            <label htmlFor="auto-type-window">{t("itemEditor.windows")}</label>
            <textarea
              id="auto-type-window"
              rows={2}
              value={windows}
              placeholder={"*Remote Desktop*"}
              spellCheck={false}
              onChange={(e) => setWindows(e.target.value)}
            />
          </div>
        </details>
      )}
      <div className="field">
        <label htmlFor="notes">{t("itemEditor.notes")}</label>
        <textarea
          id="notes"
          rows={4}
          value={notes}
          disabled={!notesLoaded}
          onChange={(e) => setNotes(e.target.value)}
        />
      </div>
      <div className="field">
        <label htmlFor="tags">{t("itemEditor.tags")}</label>
        <input id="tags" value={tags} onChange={(e) => setTags(e.target.value)} />
      </div>
      <ErrorMessage error={save.error ?? generate.error ?? attachDocument.error} />
    </form>
  );
}
