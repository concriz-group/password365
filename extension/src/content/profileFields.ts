// Address and payment forms: which field wants what, and how to fill it.
// Field names are HTML autocomplete names (`given-name`, `cc-number` …), the
// same keys the app sends values under.

import { AUTOFILL } from "virtual:autofill";

import { setValue } from "./fill";

export type ProfileKind = "identity" | "card";
export type FormField = HTMLInputElement | HTMLSelectElement;

/** One recognised field. */
export interface ProfileField {
  element: FormField;
  name: string;
}

const CARD_NAMES = new Set(["cc-name", "cc-number", "cc-csc", "cc-exp", "cc-exp-month", "cc-exp-year", "cc-type"]);
const IDENTITY_NAMES = new Set([
  "name",
  "given-name",
  "additional-name",
  "family-name",
  "email",
  "tel",
  "street-address",
  "address-line1",
  "address-line2",
  "address-level1",
  "address-level2",
  "address-level3",
  "postal-code",
  "country",
  "country-name",
  "organization",
  "organization-title",
  "bday",
  "bday-day",
  "bday-month",
  "bday-year",
]);

/** Prefix of the values that carry the item's own fields by label (see the app's `form_values`). */
export const LABEL_PREFIX = "label:";

/**
 * Guesses from the field's name, id, placeholder and label when the page sets
 * no autocomplete attribute: the patterns of `templates/autofill.json` and of
 * every country, tried in order (the first match wins). Card patterns go
 * first, whatever file they come from: a country's words for "number" must
 * never take a card number box.
 */
const HINTS: [string, RegExp][] = [...AUTOFILL]
  .sort((a, b) => Number(b.kind === "card") - Number(a.kind === "card"))
  .flatMap(({ token, hints }) => hints.map((hint) => [token, new RegExp(hint, "i")] as [string, RegExp]));
const TOKEN_KIND = new Map(AUTOFILL.map((f) => [f.token, f.kind]));

function labelText(element: FormField): string {
  const labels = element.labels ? [...element.labels].map((l) => l.textContent ?? "") : [];
  const placeholder = element instanceof HTMLInputElement ? element.placeholder : "";
  return [element.name, element.id, placeholder, element.getAttribute("aria-label") ?? "", ...labels].join(" ");
}

/** The page's autocomplete attribute (read as an attribute: not every browser reflects it on selects). */
function autocompleteOf(element: FormField): string {
  return (element.getAttribute("autocomplete") ?? "").trim().toLowerCase();
}

/** The autocomplete name a field asks for, ignoring section/shipping/billing words. */
function fromAutocomplete(element: FormField): string | null {
  const tokens = autocompleteOf(element).split(/\s+/);
  const last = tokens.at(-1) ?? "";
  return CARD_NAMES.has(last) || IDENTITY_NAMES.has(last) ? last : null;
}

function usable(element: FormField): boolean {
  if (element.disabled || (element instanceof HTMLInputElement && element.readOnly)) return false;
  if (element instanceof HTMLInputElement) {
    return ["text", "email", "tel", "number", "search", "month", "date", ""].includes(element.type);
  }
  return true;
}

/** What `element` asks for, or `null`. */
export function classify(element: FormField): string | null {
  if (!usable(element)) return null;
  const declared = fromAutocomplete(element);
  if (declared) return declared;
  // The page says the field is something else (a username, a one-time code …): believe it.
  const declaredOther = autocompleteOf(element);
  if (declaredOther && declaredOther !== "on" && declaredOther !== "off") return null;
  if (element instanceof HTMLInputElement && element.type === "email") return "email";
  if (element instanceof HTMLInputElement && element.type === "tel") return "tel";
  const text = labelText(element);
  return HINTS.find(([, pattern]) => pattern.test(text))?.[0] ?? null;
}

export const kindOf = (name: string): ProfileKind =>
  CARD_NAMES.has(name) || TOKEN_KIND.get(name) === "card" ? "card" : "identity";

/**
 * Address and payment fields of the page. Fields of a login form (one with a
 * password field) are left to the login menu. An identity needs at least
 * two fields (a lone e-mail box is a newsletter, not an address form); a card
 * needs its number.
 */
export function findProfileFields(doc: Document): Record<ProfileKind, ProfileField[]> {
  const found: Record<ProfileKind, ProfileField[]> = { identity: [], card: [] };
  for (const element of doc.querySelectorAll<FormField>("input, select")) {
    const scope = element.form ?? doc;
    if (scope.querySelector("input[type=password]")) continue;
    const name = classify(element);
    if (name) found[kindOf(name)].push({ element, name });
  }
  if (found.identity.length < 2) found.identity = [];
  if (!found.card.some((f) => f.name === "cc-number")) found.card = [];
  return found;
}

/**
 * The value for `name`, derived from what the app sent when the page wants
 * another shape. `split`: the form has its own boxes for the number or the
 * second line, so the street box gets the first line only.
 */
function valueFor(field: ProfileField, values: Record<string, string>, split: boolean): string | undefined {
  const { name, element } = field;
  if (name === "cc-exp" && values["cc-exp-month"] && values["cc-exp-year"]) {
    const placeholder = element instanceof HTMLInputElement ? element.placeholder : "";
    const long = /yyyy|aaaa/i.test(placeholder) || (element instanceof HTMLInputElement && element.maxLength >= 7);
    const year = long ? values["cc-exp-year"] : values["cc-exp-year"].slice(-2);
    return element instanceof HTMLInputElement && element.type === "month"
      ? `${values["cc-exp-year"]}-${values["cc-exp-month"]}`
      : `${values["cc-exp-month"]}/${year}`;
  }
  if (name === "street-address" || name === "address-line1") {
    const whole = values["street-address"]?.split(/\s*\n\s*/).filter(Boolean).join(", ");
    return split ? (values["address-line1"] ?? whole) : (whole ?? values["address-line1"]);
  }
  if (name === "country") return values["country"] ?? values["country-name"];
  return values[name];
}

/** Picks the option whose value or text matches (numbers compared as numbers: "04" = "4"). */
function selectOption(select: HTMLSelectElement, value: string): boolean {
  const wanted = value.trim().toLowerCase();
  const number = Number(wanted);
  const option = [...select.options].find((o) => {
    const v = o.value.trim().toLowerCase();
    const t = (o.textContent ?? "").trim().toLowerCase();
    if (v === wanted || t === wanted) return true;
    if (!Number.isNaN(number) && wanted !== "") return Number(v) === number || Number(t) === number || v.endsWith(wanted);
    return false;
  });
  if (!option) return false;
  select.value = option.value;
  select.dispatchEvent(new Event("input", { bubbles: true }));
  select.dispatchEvent(new Event("change", { bubbles: true }));
  return true;
}

/**
 * `value` as the box takes it: a formatted value ("123.456.789-09") that does
 * not fit the box's maximum length goes in as its digits.
 */
export function fitted(element: HTMLInputElement, value: string): string {
  if (element.maxLength <= 0 || value.length <= element.maxLength) return value;
  const digits = value.replace(/\D/g, "");
  return digits.length > 0 && digits.length <= element.maxLength ? digits : value;
}

function put(element: FormField, value: string): boolean {
  if (element instanceof HTMLSelectElement) return selectOption(element, value);
  setValue(element, fitted(element, value));
  return true;
}

/** Fills the fields there is a value for; returns how many were filled. */
export function fillProfile(fields: ProfileField[], values: Record<string, string>): number {
  const split = fields.some((f) => f.name === "house-number" || f.name === "address-line2");
  let filled = 0;
  for (const field of fields) {
    const value = valueFor(field, values, split);
    if (value && put(field.element, value)) filled++;
  }
  return filled;
}

/** Lower case, no accents, single spaces: "Nº  Matrícula" → "nº matricula". */
export function normalize(text: string): string {
  return text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}º]+/gu, " ")
    .trim();
}

/**
 * Fills boxes the patterns did not recognise whose label is the name of one
 * of the item's own fields ("Matrícula", "Membership number"). Returns how many.
 */
export function fillOwnFields(doc: Document, values: Record<string, string>, taken: ProfileField[]): number {
  const own = Object.entries(values)
    .filter(([key]) => key.startsWith(LABEL_PREFIX))
    .map(([key, value]) => [normalize(key.slice(LABEL_PREFIX.length)), value] as const)
    .filter(([label]) => label.length > 0);
  if (own.length === 0) return 0;
  const used = new Set<Element>(taken.map((f) => f.element));
  let filled = 0;
  for (const element of doc.querySelectorAll<FormField>("input, select")) {
    if (used.has(element) || !usable(element) || (element.form ?? doc).querySelector("input[type=password]")) continue;
    const texts = [element.name, element.id, element.getAttribute("aria-label") ?? "", ...[...(element.labels ?? [])].map((l) => l.textContent ?? "")]
      .concat(element instanceof HTMLInputElement ? [element.placeholder] : [])
      .map(normalize);
    const match = own.find(([label]) => texts.includes(label));
    if (match && put(element, match[1])) filled++;
  }
  return filled;
}
