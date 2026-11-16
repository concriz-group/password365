/** The username and password fields of a login (or sign-up) form. */
export interface LoginFields {
  form: HTMLFormElement | null;
  username: HTMLInputElement | null;
  password: HTMLInputElement;
  /** Every usable password field in the form (new + confirmation on sign-up). */
  passwords: HTMLInputElement[];
  /** Sign-up / change-password form (new password requested). */
  isSignup: boolean;
}

const USERNAME_TYPES = new Set(["text", "email", "tel", ""]);
const USERNAME_HINTS = /user|login|email|e-mail|mail|account|identifier|phone/i;

function usable(input: HTMLInputElement): boolean {
  return !input.disabled && !input.readOnly && !input.hidden && input.type !== "hidden";
}

function isPassword(input: HTMLInputElement): boolean {
  return input.type === "password" && usable(input);
}

function looksLikeUsername(input: HTMLInputElement): boolean {
  const ac = input.autocomplete.toLowerCase();
  if (ac.includes("username") || ac.includes("email")) return true;
  return USERNAME_HINTS.test(`${input.name} ${input.id} ${input.placeholder} ${input.getAttribute("aria-label") ?? ""}`);
}

/** Picks the username field: the best-looking text input before the password. */
function findUsername(candidates: HTMLInputElement[], password: HTMLInputElement): HTMLInputElement | null {
  const before = candidates.filter(
    (i) =>
      USERNAME_TYPES.has(i.type) &&
      usable(i) &&
      // password comes after this input in document order
      (i.compareDocumentPosition(password) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
  );
  return [...before].reverse().find(looksLikeUsername) ?? before.at(-1) ?? null;
}

/** Finds the login fields in a document, or `null` if there is no password field. */
export function findLoginFields(doc: Document): LoginFields | null {
  const passwords = [...doc.querySelectorAll<HTMLInputElement>("input")].filter(isPassword);
  const password = passwords[0];
  if (!password) return null;
  const form = password.form;
  const scope = [...(form ?? doc).querySelectorAll<HTMLInputElement>("input")];
  const passwordsInScope = scope.filter(isPassword);
  const isSignup =
    passwordsInScope.length >= 2 || passwordsInScope.some((p) => p.autocomplete.toLowerCase().includes("new-password"));
  return { form, username: findUsername(scope, password), password, passwords: passwordsInScope, isSignup };
}
