import { findLoginFields } from "./detect";

export interface Submission {
  username?: string;
  password: string;
}

/** Ignore repeated captures of the same submit (click + submit event) within this window. */
export const DEDUPE_MS = 1_000;

/**
 * Whether an event is the user's own: trusted (not dispatched by a script)
 * and, where the browser tells, right after a real click or key press. A page
 * that could fire submissions itself could test password guesses against the
 * vault by watching whether the "save login" prompt appears.
 */
export function byUser(e: Event): boolean {
  const activation = (navigator as { userActivation?: { isActive: boolean } }).userActivation;
  return e.isTrusted && (activation?.isActive ?? true);
}

/**
 * Calls `onSubmit` when the user submits a login form: form `submit` events,
 * clicks on submit buttons and Enter in the password field (single-page apps
 * often never fire a real submit). Only the user's own actions count (see
 * {@link byUser}). Returns an unsubscribe function.
 */
export function watchSubmissions(
  doc: Document,
  onSubmit: (submission: Submission) => void,
  now: () => number = Date.now,
  isUser: (e: Event) => boolean = byUser,
): () => void {
  let last = { key: "", at: -Infinity };

  function capture() {
    const fields = findLoginFields(doc);
    const password = fields?.password.value ?? "";
    if (!fields || !password) return;
    const username = fields.username?.value.trim() || undefined;
    const key = `${username ?? ""}\u0000${password}`;
    if (key === last.key && now() - last.at < DEDUPE_MS) return;
    last = { key, at: now() };
    onSubmit({ username, password });
  }

  const onSubmitEvent = (e: Event) => {
    if (isUser(e)) capture();
  };
  const onClick = (e: Event) => {
    if (!isUser(e)) return;
    const target = e.target as Element | null;
    const button = target?.closest?.("button, input[type=submit]");
    if (!button) return;
    const type = (button.getAttribute("type") ?? "submit").toLowerCase();
    if (type === "submit") capture();
  };
  const onKey = (e: KeyboardEvent) => {
    if (isUser(e) && e.key === "Enter" && (e.target as HTMLInputElement | null)?.type === "password") capture();
  };

  doc.addEventListener("submit", onSubmitEvent, true);
  doc.addEventListener("click", onClick, true);
  doc.addEventListener("keydown", onKey, true);
  return () => {
    doc.removeEventListener("submit", onSubmitEvent, true);
    doc.removeEventListener("click", onClick, true);
    doc.removeEventListener("keydown", onKey, true);
  };
}
