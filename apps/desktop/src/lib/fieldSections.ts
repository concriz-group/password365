// The editor's fields grouped in sections, as 1Password shows them: the
// category's sections, fields the user adds with "add another field", and
// sections added with "add more".

import type { Category, FieldInput, FieldSpec, FieldValue, ItemField } from "../api/types";

/** One editable field. */
export interface FieldRow {
  id: string;
  label: string;
  input: FieldInput;
  concealed: boolean;
  value: string;
  /** Id of the row's [`Section`]. */
  sectionId: string;
  /** What the country template says (shown name, mask, check, choices). */
  spec?: Pick<FieldSpec, "name" | "mask" | "validate" | "options">;
  /** Added by the user: the label can be changed and the field removed. */
  custom: boolean;
}

/** A block of fields; `name` "" is the item's first block (no heading). */
export interface Section {
  id: string;
  name: string;
  /** Added with "add more": the name can be changed. */
  custom: boolean;
}

/** A kind of field "add another field" offers; `key` names it in the `fieldTypes` catalog. */
export interface FieldType {
  key: string;
  /** English name (the catalog has the translations). */
  name: string;
  input: FieldInput;
  concealed: boolean;
}

/** The kinds of field "add another field" offers. */
export const FIELD_TYPES: FieldType[] = [
  { key: "text", name: "Text", input: "text", concealed: false },
  { key: "url", name: "URL", input: "url", concealed: false },
  { key: "email", name: "Email", input: "email", concealed: false },
  { key: "address", name: "Address", input: "multiline", concealed: false },
  { key: "date", name: "Date", input: "date", concealed: false },
  { key: "totp", name: "One-Time Password", input: "totp", concealed: false },
  { key: "password", name: "Password", input: "text", concealed: true },
  { key: "phone", name: "Phone", input: "phone", concealed: false },
  { key: "signInWith", name: "Sign in with", input: "sign_in_with", concealed: false },
];

let next = 0;
export const newId = (prefix: string) => `${prefix}-${++next}`;

/** The fields `category` has in the country `region` (its shared ones when the country has none). */
export function templateIn(category: Category | undefined, region: string): FieldSpec[] {
  return category?.regions?.[region] ?? category?.fields ?? [];
}

/** The template field called `label` (as the vault compares labels). */
export function specOf(category: Category | undefined, region: string, label: string): FieldSpec | undefined {
  const wanted = label.toLowerCase();
  return templateIn(category, region).find((f) => f.label.toLowerCase() === wanted);
}

/** The countries with their own fields for `category`. */
export const regionsOf = (category: Category | undefined): string[] => Object.keys(category?.regions ?? {}).sort();

/** The region the system's locales point at ("pt-BR" → "BR"), or "". */
export function systemRegion(locales: readonly string[]): string {
  for (const tag of locales) {
    try {
      const region = new Intl.Locale(tag).region;
      if (region) return region;
    } catch {
      // not a locale tag: try the next one
    }
  }
  return "";
}

/**
 * The country a new item of `category` uses: the one chosen in Settings
 * (never the interface language), else the system's region. "" (the shared
 * fields) when the category has nothing special for that country.
 */
export function defaultRegion(category: Category | undefined, chosen: string, locales: readonly string[]): string {
  const region = chosen || systemRegion(locales);
  return regionsOf(category).includes(region) ? region : "";
}

/** The rows as item fields (to rebuild them on another country's template). */
export function asItemFields(rows: FieldRow[], sections: Section[]): ItemField[] {
  return rows
    .filter((r) => r.value !== "")
    .map((r) => ({
      label: r.label,
      value: r.value,
      concealed: r.concealed,
      input: r.input,
      section: sections.find((s) => s.id === r.sectionId)?.name ?? "",
    }));
}

/**
 * The rows and sections to edit: the category's fields (with the item's
 * values), then the item's own fields, each at the end of its section.
 */
export function initialFields(category: Category | undefined, fields: ItemField[]): { rows: FieldRow[]; sections: Section[] } {
  const templateSections = new Set(category?.fields.map((f) => f.section));
  const sections: Section[] = [];
  const sectionId = (name: string) => {
    let section = sections.find((s) => s.name === name);
    if (!section) {
      section = { id: newId("section"), name, custom: name !== "" && !templateSections.has(name) };
      sections.push(section);
    }
    return section.id;
  };
  const valueOf = (label: string) => fields.find((f) => f.label === label)?.value ?? "";
  let rows: FieldRow[] = (category?.fields ?? []).map((f) => ({
    label: f.label,
    input: f.input,
    concealed: f.concealed,
    spec: { name: f.name, mask: f.mask, validate: f.validate, options: f.options },
    id: newId("field"),
    value: valueOf(f.label),
    sectionId: sectionId(f.section),
    custom: false,
  }));
  for (const f of fields.filter((f) => !rows.some((r) => r.label === f.label))) {
    const row = { label: f.label, input: f.input, concealed: f.concealed, value: f.value ?? "", custom: true };
    rows = insertAtEnd(rows, { ...row, id: newId("field"), sectionId: sectionId(f.section) });
  }
  if (sections.length === 0) sectionId("");
  return { rows, sections };
}

function insertAtEnd(rows: FieldRow[], row: FieldRow): FieldRow[] {
  const last = rows.map((r) => r.sectionId).lastIndexOf(row.sectionId);
  const copy = [...rows];
  copy.splice(last < 0 ? rows.length : last + 1, 0, row);
  return copy;
}

/** A label not used by any row yet: "phone", "phone 2", … */
export function freeLabel(rows: FieldRow[], base: string): string {
  const taken = new Set(rows.map((r) => r.label.trim().toLowerCase()));
  if (!taken.has(base)) return base;
  let n = 2;
  while (taken.has(`${base} ${n}`)) n++;
  return `${base} ${n}`;
}

/**
 * A new field of `type` at the end of the section `sectionId`, labelled
 * `name` (the type's name in the user's language) or a free variant of it.
 */
export function addField(rows: FieldRow[], sectionId: string, type: FieldType, name = type.name): FieldRow[] {
  return insertAtEnd(rows, {
    id: newId("field"),
    label: freeLabel(rows, name.toLowerCase()),
    input: type.input,
    concealed: type.concealed,
    value: "",
    sectionId,
    custom: true,
  });
}

/** A label used by two fields (compared as the vault does), or `null`. */
export function duplicateLabel(rows: FieldRow[]): string | null {
  const seen = new Set<string>();
  for (const r of rows.filter((r) => r.value !== "" && r.label.trim() !== "")) {
    const key = r.label.trim().toLowerCase();
    if (seen.has(key)) return r.label.trim();
    seen.add(key);
  }
  return null;
}

/** What the editor saves: every row, placed in its section. */
export function toFieldValues(rows: FieldRow[], sections: Section[]): FieldValue[] {
  return rows.map(({ label, value, sectionId, input, concealed }) => ({
    label: label.trim(),
    value,
    section: (sections.find((s) => s.id === sectionId)?.name ?? "").trim(),
    input,
    concealed,
  }));
}

/** `items` grouped by `key`, in the order of `keys` (kept even when empty), then any other key. */
export function grouped<T>(items: T[], key: (item: T) => string, keys: string[] = []): [string, T[]][] {
  const all = [...new Set([...keys, ...items.map(key)])];
  return all.map((k) => [k, items.filter((i) => key(i) === k)]);
}

/** Row `id` moved to just before row `beforeId`, into that row's section. */
export function moveBefore(rows: FieldRow[], id: string, beforeId: string): FieldRow[] {
  const moving = rows.find((r) => r.id === id);
  const target = rows.find((r) => r.id === beforeId);
  if (!moving || !target || id === beforeId) return rows;
  const rest = rows.filter((r) => r.id !== id);
  const at = rest.indexOf(target);
  return [...rest.slice(0, at), { ...moving, sectionId: target.sectionId }, ...rest.slice(at)];
}

/** Row `id` moved one place up (-1) or down (1), crossing into the next section at its edge. */
export function moveBy(rows: FieldRow[], id: string, delta: -1 | 1): FieldRow[] {
  const i = rows.findIndex((r) => r.id === id);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= rows.length) return rows;
  const copy = [...rows];
  const neighbour = copy[j]!;
  if (neighbour.sectionId !== copy[i]!.sectionId) {
    copy[i] = { ...copy[i]!, sectionId: neighbour.sectionId };
    return copy;
  }
  copy[j] = copy[i]!;
  copy[i] = neighbour;
  return copy;
}

