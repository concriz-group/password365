import type { LoginFields } from "./detect";

/**
 * Sets an input's value the way a user would, so frameworks (React, Vue …)
 * that track values through events notice the change.
 */
export function setValue(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  if (setter) setter.call(input, value);
  else input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

/** Fills username and password; returns `true` if anything was filled. */
export function fillLogin(
  fields: LoginFields,
  creds: { username: string | null; password: string | null },
): boolean {
  let filled = false;
  if (fields.username && creds.username) {
    setValue(fields.username, creds.username);
    filled = true;
  }
  if (creds.password) {
    setValue(fields.password, creds.password);
    filled = true;
  }
  return filled;
}

/** Puts a newly generated password into every password field of a sign-up form. */
export function fillNewPassword(fields: LoginFields, password: string): boolean {
  for (const input of fields.passwords) setValue(input, password);
  return fields.passwords.length > 0;
}

/**
 * Puts a suggested password into the field the user right-clicked and, for a
 * password field, into the first empty password field after it (the
 * confirmation). Earlier fields — e.g. "current password" — are never touched.
 */
export function fillSuggested(target: HTMLInputElement, password: string): boolean {
  if (target.disabled || target.readOnly) return false;
  setValue(target, password);
  if (target.type !== "password") return true;
  const scope = target.form ?? target.ownerDocument;
  const confirmation = [...scope.querySelectorAll<HTMLInputElement>("input[type=password]")].find(
    (input) =>
      input !== target &&
      !input.disabled &&
      !input.readOnly &&
      input.value === "" &&
      (target.compareDocumentPosition(input) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
  );
  if (confirmation) setValue(confirmation, password);
  return true;
}
