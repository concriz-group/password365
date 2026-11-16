// Auto-type settings live in two custom fields of the item (see
// `core/password365-app/src/auto_type.rs`, which owns these names).

export const SEQUENCE_LABEL = "auto-type sequence";
export const WINDOW_LABEL = "auto-type window";
export const DEFAULT_SEQUENCE = "{USERNAME}{TAB}{PASSWORD}{ENTER}";

/** True for the fields the Auto-type section shows (not the general field list). */
export function isAutoTypeField(label: string): boolean {
  const l = label.toLowerCase();
  return l === SEQUENCE_LABEL || l === WINDOW_LABEL;
}
