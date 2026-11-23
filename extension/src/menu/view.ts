import { helpFor } from "../shared/help";
import type { MenuMessage, MenuState, PasskeyPrompt, Reply } from "../shared/messages";
import type { LoginCandidate } from "../shared/protocol";
import { t } from "../shared/i18n";

export type MenuMode = "login" | "signup" | "identity" | "card" | "passkey";

export interface MenuDeps {
  send: (message: MenuMessage) => Promise<Reply>;
  mode: MenuMode;
  /** How often the locked passkey prompt checks whether the app was unlocked. */
  pollMs?: number;
}

function el<K extends keyof HTMLElementTagNameMap>(doc: Document, tag: K, text?: string, className?: string) {
  const node = doc.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}

/** A row of the menu: avatar letter, title and subtitle; runs `action` on click. */
function row(doc: Document, title: string, subtitle: string, action: () => Promise<Reply>, warning?: string) {
  const button = el(doc, "button", undefined, "row");
  button.type = "button";
  const avatar = el(doc, "span", (title.trim()[0] ?? "?").toUpperCase(), "avatar");
  const text = el(doc, "span", undefined, "text");
  text.append(el(doc, "span", title, "title"), el(doc, "span", subtitle, "sub"));
  if (warning) text.append(el(doc, "span", warning, "warn"));
  button.append(avatar, text);
  button.addEventListener("click", async () => {
    button.disabled = true;
    const reply = await action();
    if (!reply.ok) {
      text.querySelector(".sub")!.textContent = helpFor(reply.error);
      button.disabled = false;
    } else if (reply.result === "again") {
      // The click only asked for a confirmation: ready for the real one.
      button.disabled = false;
    }
  });
  return button;
}

/**
 * Renders the inline menu (1Password-style): suggested password on sign-up
 * forms, then the saved logins for the site. Text is set with `textContent`.
 */
export async function renderMenu(root: HTMLElement, { send, mode, pollMs = 1000 }: MenuDeps): Promise<void> {
  const doc = root.ownerDocument;
  root.replaceChildren();
  const note = (text: string) => root.append(el(doc, "p", text, "note"));

  if (mode === "passkey") return renderPasskey(root, send, pollMs);
  const reply = await send({ kind: "menu-state" });
  if (!reply.ok) return note(helpFor(reply.error));
  const state = reply.result as MenuState;
  if (state.state === "no_account") return note(t("createAccountFirst"));
  if (state.state === "locked") {
    root.append(row(doc, t("unlockApp"), t("enterMasterPassword"), () => send({ kind: "menu-unlock" })));
    return;
  }

  if (mode === "identity" || mode === "card") {
    const chosen = state.profile?.[mode] ?? null;
    const all = (mode === "card" ? state.profile?.cards : state.profile?.identities) ?? (chosen ? [chosen] : []);
    if (all.length === 0) return note(mode === "card" ? t("chooseCard") : t("chooseIdentity"));
    root.append(el(doc, "p", mode === "card" ? t("paymentCard") : t("identity"), "heading"));
    for (const item of all) {
      const subtitle = mode === "card" ? (item.last4 ? `•••• ${item.last4}` : t("fillCard")) : t("fillIdentity");
      root.append(row(doc, item.title, subtitle, () => send({ kind: "menu-fill-profile", profile: mode, itemId: item.id })));
    }
    return;
  }

  if (mode === "signup") {
    root.append(
      row(doc, t("useSuggested"), t("suggestedStrong"), () => send({ kind: "menu-suggest" })),
    );
  }
  // Passkeys a waiting sign-in can use (the site asked "conditionally").
  const passkeys = state.passkeys ?? [];
  if (passkeys.length > 0) root.append(el(doc, "p", t("passkeysForSite"), "heading"));
  for (const passkey of passkeys) {
    root.append(row(doc, passkey.title, t("passkeyAccount", { user: passkey.user_name }), () => send({ kind: "menu-passkey-autofill", itemId: passkey.item_id })));
  }
  const logins: LoginCandidate[] = state.logins ?? [];
  if (logins.length > 0) root.append(el(doc, "p", t("loginsForSite"), "heading"));
  for (const login of logins) {
    // A related-domain login on a site it was never used on: the first click only asks.
    let armed = login.new_site !== true;
    const button = row(
      doc,
      login.title,
      login.username ?? "",
      async () => {
        if (!armed) {
          armed = true;
          button.querySelector(".sub")!.textContent = t("confirmFillHere");
          button.classList.add("confirm");
          return { ok: true, result: "again" } as Reply;
        }
        return send({ kind: "menu-fill", itemId: login.id, ...(login.new_site ? { confirmed: true } : {}) });
      },
      login.new_site === true ? t("newSiteNote") : undefined,
    );
    button.dataset.search = `${login.title}\u0000${login.username ?? ""}`.toLowerCase();
    root.append(button);
  }
  if (logins.length === 0 && passkeys.length === 0 && mode === "login") note(t("noLogins"));
  if (logins.length > 0) root.append(el(doc, "p", t("noMatchingLogins"), "note no-match"));
  applyFilter(root, "");
}

/** The passkey prompt: save a new passkey, or choose which one to sign in with. */
async function renderPasskey(root: HTMLElement, send: MenuDeps["send"], pollMs: number): Promise<void> {
  const doc = root.ownerDocument;
  const reply = await send({ kind: "menu-passkey-state" });
  root.replaceChildren();
  if (!reply.ok) return void root.append(el(doc, "p", helpFor(reply.error), "note"));
  const prompt = reply.result as PasskeyPrompt | null;
  if (!prompt) return void root.append(el(doc, "p", t("passkeyGone"), "note"));
  if (prompt.kind === "locked") {
    root.append(el(doc, "p", t("passkeyLocked", { site: prompt.site }), "heading"));
    root.append(row(doc, t("unlockApp"), t("enterMasterPassword"), () => send({ kind: "menu-passkey-unlock" })));
    // Carry on by itself once the app is unlocked (in the popup or the app).
    setTimeout(() => void renderPasskey(root, send, pollMs), pollMs);
  } else if (prompt.kind === "verify") {
    // The site requires the user verified: the master password is typed in the popup.
    root.append(el(doc, "p", t("passkeyVerify", { site: prompt.site }), "heading"));
    root.append(row(doc, t("confirmIdentity"), t("enterMasterPassword"), () => send({ kind: "menu-passkey-unlock" })));
    setTimeout(() => void renderPasskey(root, send, pollMs), pollMs);
  } else {
    // After a choice the site may want the user verified first: show that.
    const choose = async (itemId?: string) => {
      const reply = await send({ kind: "menu-passkey-choose", itemId });
      if (reply.ok && reply.result === "verify") await renderPasskey(root, send, pollMs);
      return reply;
    };
    if (prompt.kind === "create") {
      root.append(el(doc, "p", t("passkeySaveFor", { site: prompt.site }), "heading"));
      root.append(row(doc, t("passkeySave"), prompt.user_name, () => choose()));
    } else {
      root.append(el(doc, "p", t("passkeySignInTo", { site: prompt.site }), "heading"));
      for (const choice of prompt.choices) {
        root.append(row(doc, choice.title, choice.user_name, () => choose(choice.item_id)));
      }
    }
  }
  root.append(row(doc, t("passkeyOther"), t("passkeyOtherHint"), () => send({ kind: "menu-passkey-other" })));
}

/**
 * Shows only the logins whose title or username contains `text` (what the
 * user is typing in the username field), like 1Password.
 */
export function applyFilter(root: HTMLElement, text: string): void {
  const needle = text.trim().toLowerCase();
  let shown = 0;
  for (const button of root.querySelectorAll<HTMLElement>("[data-search]")) {
    const match = !needle || button.dataset.search!.includes(needle);
    button.hidden = !match;
    if (match) shown++;
  }
  const noMatch = root.querySelector<HTMLElement>(".no-match");
  if (noMatch) noMatch.hidden = shown > 0;
}

/** Arrow keys move between the visible rows (Enter clicks the focused one natively). */
export function moveFocus(root: HTMLElement, key: string): boolean {
  if (key !== "ArrowDown" && key !== "ArrowUp") return false;
  const rows = [...root.querySelectorAll<HTMLButtonElement>("button.row")].filter((b) => !b.hidden);
  if (rows.length === 0) return false;
  const current = rows.indexOf(root.ownerDocument.activeElement as HTMLButtonElement);
  const next =
    current === -1 ? 0 : key === "ArrowDown" ? Math.min(rows.length - 1, current + 1) : Math.max(0, current - 1);
  rows[next]!.focus();
  return true;
}
