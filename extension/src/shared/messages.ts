import type { LoginCandidate, Profile, ProfileKind } from "./protocol";
import type { CreateOptions, GetOptions } from "../passkey/codec";
// Messages between the extension's own parts (popup, content script, background).

/** Sent by the popup (an extension page — never by web content). */
export type PopupMessage =
  | { kind: "status" }
  | { kind: "unlock"; password: string }
  | { kind: "logins"; tabId: number }
  | { kind: "fill"; tabId: number; itemId: string }
  | { kind: "generate" }
  | { kind: "passkey-waiting"; tabId: number };

/** Sent by the content script; the page URL is taken from the sender, never from the message. */
export type ContentMessage =
  | { kind: "submitted"; username?: string; password: string; title?: string }
  | { kind: "pending" }
  | { kind: "save-pending" }
  | { kind: "dismiss-pending" }
  | { kind: "passkey-create"; options: CreateOptions }
  | { kind: "passkey-get"; options: GetOptions }
  | { kind: "passkey-conditional"; options: GetOptions }
  | { kind: "passkey-abort" }
  /** The content script opened its inline menu with this one-time `token` in the menu's URL. */
  | { kind: "menu-opened"; token: string };

/**
 * Sent by the inline menu (an extension page in an iframe the content script
 * places under a login field). The tab comes from the sender.
 */
export type MenuMessage =
  | { kind: "menu-state" }
  | { kind: "menu-fill"; itemId: string }
  | { kind: "menu-suggest" }
  | { kind: "menu-close" }
  | { kind: "menu-unlock" }
  | { kind: "menu-resize"; height: number }
  | { kind: "menu-fill-profile"; profile: ProfileKind; itemId?: string }
  | { kind: "menu-passkey-state" }
  | { kind: "menu-passkey-choose"; itemId?: string }
  | { kind: "menu-passkey-other" }
  | { kind: "menu-passkey-unlock" }
  | { kind: "menu-passkey-autofill"; itemId: string };

/** Every menu message carries the token of the menu the content script opened. */
export type FromMenu = MenuMessage & { token: string };

/** A login that holds a passkey for the page (no secrets). */
export interface PasskeyChoice {
  item_id: string;
  title: string;
  user_name: string;
}

/** What the passkey prompt shows. */
export type PasskeyPrompt =
  | { kind: "create"; site: string; user_name: string }
  | { kind: "get"; site: string; choices: PasskeyChoice[] }
  | { kind: "locked"; site: string };

/** What the inline menu shows. */
export type MenuState =
  | { state: "no_account" }
  | { state: "locked" }
  | { state: "ready"; logins: LoginCandidate[]; profile?: Profile | null; passkeys?: PasskeyChoice[] };

/** Sent by the background to a tab's content script. */
export type TabMessage =
  | { kind: "fill"; username: string | null; password: string | null; fromMenu?: boolean }
  | { kind: "fill-new-password"; password: string; fromMenu?: boolean }
  | { kind: "suggest-at-target"; password: string }
  | { kind: "close-menu" }
  | { kind: "open-menu" }
  | { kind: "open-passkey" }
  | { kind: "passkey-fields"; active: boolean }
  | { kind: "resize-menu"; height: number }
  | { kind: "fill-profile"; profile: ProfileKind; values: Record<string, string>; fromMenu?: boolean };

export type Reply<T = unknown> = { ok: true; result: T } | { ok: false; error: { code: string; message: string } };

/** What the content script needs to show a save/update prompt (never the password). */
export interface PendingPrompt {
  status: "new" | "password_changed";
  username?: string;
}

export const POPUP_KINDS = new Set(["status", "unlock", "logins", "fill", "generate", "passkey-waiting"]);
export const MENU_KINDS = new Set([
  "menu-state",
  "menu-fill",
  "menu-suggest",
  "menu-close",
  "menu-unlock",
  "menu-resize",
  "menu-fill-profile",
  "menu-passkey-state",
  "menu-passkey-choose",
  "menu-passkey-other",
  "menu-passkey-unlock",
  "menu-passkey-autofill",
]);
export const CONTENT_KINDS = new Set(["menu-opened", "submitted", "pending", "save-pending", "dismiss-pending", "passkey-create", "passkey-get", "passkey-conditional", "passkey-abort"]);
