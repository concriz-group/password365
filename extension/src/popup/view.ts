import type { PopupMessage, Reply } from "../shared/messages";
import { helpFor } from "../shared/help";
import type { AppStatus, Generated, LoginCandidate } from "../shared/protocol";
import { t } from "../shared/i18n";

export interface PopupDeps {
  send: (message: PopupMessage) => Promise<Reply>;
  tabId: number | undefined;
  copy: (text: string) => Promise<void>;
  close: () => void;
}

function el<K extends keyof HTMLElementTagNameMap>(doc: Document, tag: K, text?: string, className?: string) {
  const node = doc.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}

/** Renders the popup into `root`. All text is set with `textContent` (no HTML injection). */
export async function renderPopup(root: HTMLElement, deps: PopupDeps): Promise<void> {
  const doc = root.ownerDocument;
  root.replaceChildren();
  const message = (text: string) => root.append(el(doc, "p", text, "message"));

  const status = await deps.send({ kind: "status" });
  if (!status.ok) {
    message(helpFor(status.error));
    return;
  }
  const app = status.result as AppStatus;
  if (!app.initialized) return message(t("createAccountFirst"));
  if (!app.unlocked) return renderUnlock(root, deps);
  if (deps.tabId !== undefined) {
    const verifying = await deps.send({ kind: "passkey-verifying", tabId: deps.tabId });
    if (verifying.ok && verifying.result === true) return renderVerify(root, deps, deps.tabId);
  }

  root.append(el(doc, "h1", t("loginsForSite")));
  const list = el(doc, "ul", undefined, "logins");
  root.append(list);
  await renderLogins(list, deps);
  renderGenerator(root, deps);
}

/** The page's passkey needs the user verified: the master password, typed here. */
function renderVerify(root: HTMLElement, deps: PopupDeps, tabId: number) {
  passwordForm(root, t("confirmPasskey"), t("confirm"), async (password) => {
    const reply = await deps.send({ kind: "passkey-verify", tabId, password });
    if (reply.ok) deps.close();
    return reply;
  });
}

/** Master password form (the Secret Key is already on this device). */
function renderUnlock(root: HTMLElement, deps: PopupDeps) {
  passwordForm(root, t("locked"), t("unlock"), async (password) => {
    const reply = await deps.send({ kind: "unlock", password });
    if (reply.ok) {
      // A page's passkey prompt waited for this unlock: get out of its way.
      if (deps.tabId !== undefined) {
        const waiting = await deps.send({ kind: "passkey-waiting", tabId: deps.tabId });
        if (waiting.ok && waiting.result === true) {
          deps.close();
          return reply;
        }
      }
      await renderPopup(root, deps);
    }
    return reply;
  });
}

/** A master password form; `submit` reports how it went (errors are shown here). */
function passwordForm(root: HTMLElement, heading: string, label: string, submit: (password: string) => Promise<Reply>) {
  const doc = root.ownerDocument;
  const form = el(doc, "form", undefined, "unlock");
  const input = el(doc, "input");
  input.type = "password";
  input.autocomplete = "current-password";
  input.placeholder = t("masterPassword");
  input.setAttribute("aria-label", t("masterPassword"));
  input.autofocus = true;
  const button = el(doc, "button", label, "primary");
  button.type = "submit";
  const error = el(doc, "p", undefined, "error");
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (!input.value) return;
    button.disabled = true;
    const reply = await submit(input.value);
    input.value = "";
    button.disabled = false;
    if (reply.ok) return;
    error.textContent = helpFor(reply.error);
    input.focus();
  });
  form.append(input, button, error);
  root.append(el(doc, "h1", heading), form);
  input.focus();
}

async function renderLogins(list: HTMLElement, deps: PopupDeps) {
  const doc = list.ownerDocument;
  if (deps.tabId === undefined) {
    list.append(el(doc, "li", t("noPage"), "muted"));
    return;
  }
  const tabId = deps.tabId;
  const reply = await deps.send({ kind: "logins", tabId });
  if (!reply.ok) {
    list.append(el(doc, "li", reply.error.message, "muted"));
    return;
  }
  const logins = (reply.result as LoginCandidate[] | null) ?? [];
  if (logins.length === 0) list.append(el(doc, "li", t("noLogins"), "muted"));
  for (const login of logins) {
    const item = el(doc, "li");
    const text = el(doc, "span", undefined, "login");
    text.append(el(doc, "strong", login.title), el(doc, "span", login.username ?? "", "muted"));
    const fill = el(doc, "button", t("fill"));
    fill.addEventListener("click", async () => {
      const result = await deps.send({ kind: "fill", tabId, itemId: login.id });
      if (result.ok) deps.close();
      else fill.textContent = helpFor(result.error);
    });
    item.append(text, fill);
    list.append(item);
  }
}

function renderGenerator(root: HTMLElement, deps: PopupDeps) {
  const doc = root.ownerDocument;
  const box = el(doc, "section", undefined, "generator");
  const output = el(doc, "code");
  const generate = el(doc, "button", t("generatePassword"));
  const copy = el(doc, "button", t("copy"));
  copy.hidden = true;
  generate.addEventListener("click", async () => {
    const reply = await deps.send({ kind: "generate" });
    output.textContent = reply.ok ? (reply.result as Generated).value : helpFor(reply.error);
    copy.hidden = !reply.ok;
  });
  copy.addEventListener("click", async () => {
    await deps.copy(output.textContent ?? "");
    copy.textContent = t("copied");
  });
  box.append(generate, output, copy);
  root.append(box);
}
