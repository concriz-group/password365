import {
  CONTENT_KINDS,
  type ContentMessage,
  MENU_KINDS,
  type MenuMessage,
  type MenuState,
  type PendingPrompt,
  POPUP_KINDS,
  type PopupMessage,
} from "../shared/messages";
import {
  type AppStatus,
  type LoginCandidate,
  Password365Error,
  type Profile,
  type SubmissionCheck,
  type SubmittedLogin,
} from "../shared/protocol";
import { createActions, type TabsApi } from "./actions";
import { createPasskeys } from "./passkeys";
import type { Bridge } from "./bridge";

/** Who sent a runtime message (subset of `chrome.runtime.MessageSender`). */
export interface Sender {
  id?: string;
  url?: string;
  frameId?: number;
  tab?: { id?: number };
}

export type { TabsApi };

export interface StoredPrompt {
  login: SubmittedLogin;
  status: PendingPrompt["status"];
  expires: number;
}

/** Per-tab pending save prompts (backed by `chrome.storage.session` = memory only). */
export interface PromptStore {
  get(tabId: number): Promise<StoredPrompt | undefined>;
  set(tabId: number, prompt: StoredPrompt): Promise<void>;
  delete(tabId: number): Promise<void>;
}

/** Per-tab token of the inline menu the content script opened (memory only). */
export interface MenuTokens {
  get(tabId: number): Promise<string | undefined>;
  set(tabId: number, token: string): Promise<void>;
  delete(tabId: number): Promise<void>;
}

export const PROMPT_TTL_MS = 5 * 60_000;
/** Path of the inline menu page (see `static/menu.html`). */
export const MENU_PATH = "/menu.html";

interface Deps {
  extensionId: string;
  /** This extension's own origin (`chrome.runtime.getURL("/")`). */
  extensionOrigin: string;
  bridge: Pick<Bridge, "request">;
  tabs: TabsApi;
  prompts: PromptStore;
  menus: MenuTokens;
  /** Opens the toolbar popup (where the master password is typed). */
  openPopup: () => Promise<void>;
  now?: () => number;
}

/**
 * Routes extension messages. Security rules enforced here:
 * - only this extension may send messages;
 * - popup-only actions are refused when they come from a tab (content script);
 * - content-script actions use the sender's URL from the browser, never one
 *   supplied in the message, and only from the top frame;
 * - inline-menu actions (fill, suggest) are accepted only from this
 *   extension's own `menu.html` frame carrying the one-time token the content
 *   script handed to the menu it opened, for as long as that menu is open (a
 *   page can frame `menu.html` itself, but cannot know the token); the page URL comes from the browser's
 *   record of the tab;
 * - fills asked by the menu are marked `fromMenu`: the content script applies
 *   them only while its menu is really visible (no clickjacking).
 */
export function createHandler({ extensionId, extensionOrigin, bridge, tabs, prompts, menus, openPopup, now = Date.now }: Deps) {
  const { tabUrl, fillTab, generate } = createActions({ bridge, tabs });
  const passkeys = createPasskeys({ bridge, tabs, openPopup });
  async function fromPopup(message: PopupMessage): Promise<unknown> {
    switch (message.kind) {
      case "status":
        return bridge.request<AppStatus>({ type: "status" });
      case "unlock":
        await bridge.request({ type: "unlock", password: message.password });
        return true;
      case "logins":
        return bridge.request<LoginCandidate[]>({ type: "logins_for_url", page_url: await tabUrl(message.tabId) });
      case "fill":
        return fillTab(message.tabId, message.itemId);
      case "generate":
        return generate();
      case "passkey-waiting":
        return passkeys.waiting(message.tabId);
    }
  }

  async function fromMenu(message: MenuMessage, tabId: number): Promise<unknown> {
    switch (message.kind) {
      case "menu-state": {
        const status = await bridge.request<AppStatus>({ type: "status" });
        if (!status.initialized) return { state: "no_account" } satisfies MenuState;
        if (!status.unlocked) return { state: "locked" } satisfies MenuState;
        const logins = await bridge.request<LoginCandidate[]>({ type: "logins_for_url", page_url: await tabUrl(tabId) });
        // An older app has no Profile: the menu then just has no profile rows.
        const profile = await bridge.request<Profile>({ type: "profile" }).catch(() => null);
        return { state: "ready", logins, profile, passkeys: passkeys.offeredChoices(tabId) } satisfies MenuState;
      }
      case "menu-fill":
        return fillTab(tabId, message.itemId, true);
      case "menu-suggest": {
        const { value } = await generate();
        await tabs.send(tabId, { kind: "fill-new-password", password: value, fromMenu: true });
        return true;
      }
      case "menu-close":
        // Closing the passkey prompt refuses the page's request.
        passkeys.cancel(tabId);
        await tabs.send(tabId, { kind: "close-menu" });
        return true;
      case "menu-passkey-state":
        return passkeys.prompt(tabId);
      case "menu-passkey-choose":
        return passkeys.choose(tabId, typeof message.itemId === "string" ? message.itemId : undefined);
      case "menu-passkey-other":
        return passkeys.other(tabId);
      case "menu-passkey-autofill":
        return passkeys.autofill(tabId, String(message.itemId));
      case "menu-passkey-unlock":
        return passkeys.unlock().catch(() => {
          throw new Password365Error("open_popup", "Click the Password365 icon in the toolbar to unlock");
        });
      case "menu-unlock":
        // Like 1Password: the master password is typed in the extension popup, never in the page.
        await tabs.send(tabId, { kind: "close-menu" });
        await openPopup().catch(() => {
          throw new Password365Error("open_popup", "Click the Password365 icon in the toolbar to unlock");
        });
        return true;
      case "menu-fill-profile": {
        const profile = message.profile === "card" ? "card" : "identity";
        // The app refuses card details for pages that are not https.
        const values = await bridge.request<Record<string, string>>({
          type: "profile_fill",
          profile,
          item_id: typeof message.itemId === "string" ? message.itemId : null,
          page_url: await tabUrl(tabId),
        });
        await tabs.send(tabId, { kind: "fill-profile", profile, values, fromMenu: true });
        return true;
      }
      case "menu-resize":
        await tabs.send(tabId, { kind: "resize-menu", height: Number(message.height) || 0 });
        return true;
    }
  }


  /** `true` if `url` is this extension's inline menu page. */
  function isMenuPage(url: string | undefined): boolean {
    if (!url || !URL.canParse(url)) return false;
    const parsed = new URL(url);
    // chrome-extension://<id>/ in Chromium, moz-extension://<uuid>/ in Firefox.
    // (`origin` is "null" for these schemes, so compare scheme and host.)
    const own = new URL(extensionOrigin);
    return parsed.protocol === own.protocol && parsed.host === own.host && parsed.pathname === MENU_PATH;
  }

  async function fromContent(message: ContentMessage, tabId: number, pageUrl: string): Promise<unknown> {
    switch (message.kind) {
      case "menu-opened":
        if (typeof message.token !== "string" || message.token.length < 16) return false;
        await menus.set(tabId, message.token);
        return true;
      case "menu-closed":
        // Only the menu that is closing (a newer one may already be open).
        if ((await menus.get(tabId)) !== message.token) return false;
        await menus.delete(tabId);
        return true;
      case "submitted": {
        if (!message.password) return null;
        const login: SubmittedLogin = {
          page_url: pageUrl,
          username: message.username,
          password: message.password,
          title: message.title,
        };
        const check = await bridge.request<SubmissionCheck>({ type: "check_submitted", login });
        if (check.status !== "known") {
          await prompts.set(tabId, { login, status: check.status, expires: now() + PROMPT_TTL_MS });
        }
        return check.status;
      }
      case "pending": {
        const prompt = await livePrompt(tabId);
        return prompt ? ({ status: prompt.status, username: prompt.login.username } satisfies PendingPrompt) : null;
      }
      case "save-pending": {
        const prompt = await livePrompt(tabId);
        if (!prompt) return null;
        await prompts.delete(tabId);
        await bridge.request({ type: "save_submitted", login: prompt.login });
        return true;
      }
      case "dismiss-pending":
        await prompts.delete(tabId);
        return true;
      case "passkey-create":
        return passkeys.request(tabId, pageUrl, "create", message.options);
      case "passkey-get":
        return passkeys.request(tabId, pageUrl, "get", message.options);
      case "passkey-conditional":
        return passkeys.conditional(tabId, pageUrl, message.options);
      case "passkey-abort":
        passkeys.abort(tabId);
        return true;
    }
  }

  async function livePrompt(tabId: number) {
    const prompt = await prompts.get(tabId);
    if (prompt && prompt.expires > now()) return prompt;
    if (prompt) await prompts.delete(tabId);
    return undefined;
  }


  return async function handle(message: { kind?: unknown; [field: string]: unknown }, sender: Sender): Promise<unknown> {
    const kind = message?.kind;
    if (sender.id !== extensionId || typeof kind !== "string") {
      throw new Password365Error("forbidden", "unknown sender");
    }
    const tabId = sender.tab?.id;
    if (tabId === undefined) {
      if (!POPUP_KINDS.has(kind)) throw new Password365Error("forbidden", "not allowed from here");
      return fromPopup(message as unknown as PopupMessage);
    }
    if (MENU_KINDS.has(kind)) {
      if (!isMenuPage(sender.url)) throw new Password365Error("forbidden", "not allowed from here");
      const token = await menus.get(tabId);
      if (!token || message.token !== token) throw new Password365Error("forbidden", "not our menu");
      return fromMenu(message as unknown as MenuMessage, tabId);
    }
    if (!CONTENT_KINDS.has(kind) || sender.frameId !== 0 || !sender.url) {
      throw new Password365Error("forbidden", "not allowed from a web page");
    }
    return fromContent(message as unknown as ContentMessage, tabId, sender.url);
  };
}
