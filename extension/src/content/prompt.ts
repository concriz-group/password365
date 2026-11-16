import type { PendingPrompt } from "../shared/messages";
import { createOverlay } from "./overlay";
import { t } from "../shared/i18n";

export const PROMPT_HOST_ID = "password365-save-prompt";

interface Actions {
  onSave: () => void;
  onDismiss: () => void;
}

const STYLE = `
  .bar { position: fixed; top: 16px; right: 16px; z-index: 2147483647; max-width: 340px;
         font: 14px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
         background: #1c1c1e; color: #f3f1ed; border-radius: 10px; padding: 14px 16px;
         box-shadow: 0 8px 30px rgba(0,0,0,.35); }
  .title { font-weight: 600; margin-bottom: 4px; }
  .sub { color: #a8a49c; margin-bottom: 10px; word-break: break-all; }
  button { font: inherit; border-radius: 8px; padding: 6px 12px; border: 1px solid #2e2d2b;
           background: transparent; color: inherit; cursor: pointer; margin-right: 6px; }
  button.primary { background: #ff8a1f; color: #121214; border-color: #ff8a1f; }
`;

/**
 * Shows the "Save / Update login?" bar in a **closed** shadow root so the page
 * cannot read or restyle it. Returns the (closed) root — only to our own code —
 * and a function that removes the prompt.
 */
export function showSavePrompt(
  doc: Document,
  prompt: PendingPrompt,
  actions: Actions,
): { root: ShadowRoot; remove: () => void } {
  const { root, remove } = createOverlay(doc, PROMPT_HOST_ID, STYLE);
  const bar = doc.createElement("div");
  bar.className = "bar";
  bar.setAttribute("role", "dialog");

  const title = doc.createElement("div");
  title.className = "title";
  title.textContent = prompt.status === "new" ? t("saveLogin") : t("updatePassword");
  const sub = doc.createElement("div");
  sub.className = "sub";
  sub.textContent = prompt.username ?? "";

  const save = doc.createElement("button");
  save.className = "primary";
  save.textContent = prompt.status === "new" ? t("save") : t("update");
  const dismiss = doc.createElement("button");
  dismiss.textContent = t("notNow");

  // The closed shadow root keeps page scripts from reaching these buttons.
  save.addEventListener("click", () => {
    remove();
    actions.onSave();
  });
  dismiss.addEventListener("click", () => {
    remove();
    actions.onDismiss();
  });

  bar.append(title, sub, save, dismiss);
  root.append(bar);
  return { root, remove };
}
