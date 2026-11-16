import { SOURCE_LANGUAGE, translateOr } from "../../../../i18n/translate";
import { toApiError } from "../api/client";
import { useI18n } from "../i18n";

/**
 * Renders an error (or nothing) in an ARIA alert region. In English the
 * backend's own message is shown (it is the most precise); other languages
 * get the translation for the error's code, with the backend's detail where
 * the text has room for it.
 */
export function ErrorMessage({ error }: { error: unknown }) {
  const { t, language } = useI18n();
  if (!error) return null;
  const { code, message } = toApiError(error);
  const text = language === SOURCE_LANGUAGE ? message : translateOr(t, `errors.${code}`, message, { detail: message });
  return (
    <p role="alert" className="error">
      {text}
    </p>
  );
}
