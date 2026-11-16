import type { AutoTypeChoice } from "../api/types";
import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  choice: AutoTypeChoice;
  onDone: () => void;
}

/** `text` with its `{window}` placeholder replaced by the window title in bold. */
function withWindow(text: string, window: string) {
  const [before, after = ""] = text.split("{window}");
  return (
    <>
      {before}
      <strong>{window}</strong>
      {after}
    </>
  );
}

/** The auto-type shortcut found several items for the window in front (or none): pick one. */
export function AutoTypeChooser({ choice, onDone }: Props) {
  const client = useClient();
  const t = useT();
  const type = useAction(async (id: string) => {
    await client.autoType(id);
    onDone();
  });

  return (
    <section className="auto-type-chooser" aria-label={t("autoType.title")}>
      <header className="editor-header">
        <h1>{t("autoType.title")}</h1>
        <div className="editor-actions">
          <button type="button" className="ghost" onClick={onDone}>
            {t("common.cancel")}
          </button>
        </div>
      </header>
      {choice.candidates.length === 0 ? (
        <p>{withWindow(t("autoType.noItem"), choice.window)}</p>
      ) : (
        <>
          <p>{withWindow(t("autoType.choose"), choice.window)}</p>
          <ul className="finding-items">
            {choice.candidates.map((c) => (
              <li key={c.id}>
                <button type="button" className="row" disabled={type.busy} onClick={() => void type.run(c.id)}>
                  <span className="row-title">{c.title}</span>
                  {c.username && <span className="muted">{c.username}</span>}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
      <ErrorMessage error={type.error} />
    </section>
  );
}
