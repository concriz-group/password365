import { tOr } from "./i18n";

/** A user-facing message for an error code, in the browser's language (falls back to the error's own message). */
export function helpFor(error: { code: string; message: string }): string {
  return tOr(`error_${error.code}`, error.message);
}
