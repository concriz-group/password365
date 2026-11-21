import type { PopupMessage, Reply } from "../shared/messages";
import { helpFor } from "../shared/help";
import type { AppStatus, Generated, LoginCandidate } from "../shared/protocol";
import { t } from "../shared/i18n";

export interface PopupDeps {
  send: (message: PopupMessage) => Promise<Reply>;
  tabId: number | undefined;
  copy: (text: string) => Promise<void>;
  close: () => void;
  /** Opens a web page in a new tab (the download page). */
  openUrl?: (url: string) => void;
}

/** Where the app is downloaded. */
const DOWNLOAD_URL = "https://password365.app/download/";

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

  const status = await deps.send({ kind: "status" });
  if (!status.ok) return renderConnectionProblem(root, deps, status.error);
  const app = status.result as AppStatus;
  if (!app.initialized) {
    return hero(root, { title: t("createVaultTitle"), body: t("createAccountFirst"), actions: [getAppButton(doc, deps)] });
  }
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

/** Our padlock on a soft tile, in the colours of the browser's theme. */
function logoMark(doc: Document): HTMLElement {
  const mark = el(doc, "div", undefined, "mark");
  const picture = doc.createElement("picture");
  const dark = doc.createElement("source");
  dark.srcset = "icons/logo-dark.svg";
  dark.media = "(prefers-color-scheme: dark)";
  const logo = doc.createElement("img");
  logo.src = "icons/logo.svg";
  logo.alt = "";
  picture.append(dark, logo);
  mark.append(picture);
  return mark;
}

/** The mark (our padlock), a title, a few words and what to do: one message on one screen. */
function hero(root: HTMLElement, content: { title: string; body: string; actions?: HTMLElement[] }) {
  const doc = root.ownerDocument;
  const box = el(doc, "section", undefined, "hero");
  box.append(logoMark(doc), el(doc, "h2", content.title), el(doc, "p", content.body));
  if (content.actions?.length) {
    const actions = el(doc, "div", undefined, "actions");
    actions.append(...content.actions);
    box.append(actions);
  }
  root.append(box);
}

function getAppButton(doc: Document, deps: PopupDeps) {
  const button = el(doc, "button", t("getApp"), "primary");
  button.type = "button";
  button.addEventListener("click", () => deps.openUrl?.(DOWNLOAD_URL));
  return button;
}

/** The app is not reachable: say what to do, in words, with the way to get it. */
function renderConnectionProblem(root: HTMLElement, deps: PopupDeps, error: { code: string; message: string }) {
  const doc = root.ownerDocument;
  if (error.code === "app_not_running") {
    return hero(root, { title: t("appNotRunningTitle"), body: t("appNotRunningBody"), actions: [getAppButton(doc, deps)] });
  }
  if (error.code === "host_missing") {
    return hero(root, { title: t("hostMissingTitle"), body: helpFor(error), actions: [getAppButton(doc, deps)] });
  }
  hero(root, { title: "Password365", body: helpFor(error) });
}

/** The page's passkey needs the user verified: the master password, typed here. */
function renderVerify(root: HTMLElement, deps: PopupDeps, tabId: number) {
  passwordForm(root, { title: t("confirmPasskey"), body: t("enterMasterPassword") }, t("confirm"), async (password) => {
    const reply = await deps.send({ kind: "passkey-verify", tabId, password });
    if (reply.ok) deps.close();
    return reply;
  });
}

/** Master password form (the Secret Key is already on this device). */
function renderUnlock(root: HTMLElement, deps: PopupDeps) {
  passwordForm(root, { title: t("locked"), body: t("lockedBody") }, t("unlock"), async (password) => {
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
function passwordForm(
  root: HTMLElement,
  heading: { title: string; body: string },
  label: string,
  submit: (password: string) => Promise<Reply>,
) {
  const doc = root.ownerDocument;
  const form = el(doc, "form", undefined, "unlock");
  const field = el(doc, "div", undefined, "field");
  const input = el(doc, "input");
  input.type = "password";
  input.autocomplete = "current-password";
  input.placeholder = t("masterPassword");
  input.setAttribute("aria-label", t("masterPassword"));
  input.autofocus = true;
  const toggle = el(doc, "button", t("showPassword"), "toggle");
  toggle.type = "button";
  toggle.addEventListener("click", () => {
    const hidden = input.type === "password";
    input.type = hidden ? "text" : "password";
    toggle.textContent = hidden ? t("hidePassword") : t("showPassword");
    input.focus();
  });
  field.append(input, toggle);
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
  form.append(field, button, error);
  const box = el(doc, "section", undefined, "hero");
  box.append(logoMark(doc), el(doc, "h2", heading.title), el(doc, "p", heading.body), form);
  root.append(box);
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
  if (logins.length === 0) {
    const empty = el(doc, "li", undefined, "empty");
    empty.append(el(doc, "strong", t("noLoginsTitle")), el(doc, "span", `${t("noLogins")} ${t("noLoginsHint")}`));
    list.append(empty);
  }
  for (const login of logins) {
    const item = el(doc, "li");
    const avatar = el(doc, "span", (login.title.trim()[0] ?? "?").toUpperCase(), "avatar");
    avatar.style.setProperty("--h", String(hueOf(login.title)));
    const text = el(doc, "span", undefined, "login");
    text.append(el(doc, "strong", login.title), el(doc, "span", login.username ?? "", "muted"));
    const fill = el(doc, "button", t("fill"));
    fill.addEventListener("click", async () => {
      const result = await deps.send({ kind: "fill", tabId, itemId: login.id });
      if (result.ok) deps.close();
      else fill.textContent = helpFor(result.error);
    });
    item.append(avatar, text, fill);
    list.append(item);
  }
}

/** A steady colour for a title's avatar (the same title, the same colour). */
function hueOf(title: string): number {
  let hash = 0;
  for (const c of title) hash = (hash * 31 + c.charCodeAt(0)) % 360;
  return hash;
}

function renderGenerator(root: HTMLElement, deps: PopupDeps) {
  const doc = root.ownerDocument;
  const box = el(doc, "section", undefined, "generator");
  box.append(el(doc, "h1", t("genTitle")));
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
