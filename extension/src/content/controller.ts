import type { ContentMessage, PendingPrompt, Reply, TabMessage } from "../shared/messages";
import { findLoginFields } from "./detect";
import { fillLogin, fillNewPassword, fillSuggested } from "./fill";
import { createInline, type InlineOptions } from "./inline";
import { fillOwnFields, fillProfile, findProfileFields } from "./profileFields";
import { showSavePrompt } from "./prompt";
import { watchSubmissions } from "./submit";

/** Sends a message to the background and unwraps the reply (`undefined` on failure). */
export type SendToBackground = (message: ContentMessage) => Promise<Reply | undefined>;

/**
 * Wires the content script: reports form submissions, shows a pending
 * save/update prompt, puts the inline menu on login fields, and fills fields
 * when the background asks.
 */
export function startContent(doc: Document, send: SendToBackground, inlineOptions: InlineOptions) {
  const inline = createInline(doc, {
    ...inlineOptions,
    registerMenu: (token) => send({ kind: "menu-opened", token }),
  });
  // The field the user last right-clicked (target of Password365 › Suggest password).
  let contextTarget: HTMLInputElement | null = null;
  const onContextMenu = (e: Event) => {
    contextTarget = e.target instanceof HTMLInputElement ? e.target : null;
  };
  doc.addEventListener("contextmenu", onContextMenu, true);
  const call = async (message: ContentMessage) => {
    const reply = await send(message).catch(() => undefined);
    return reply?.ok ? reply.result : undefined;
  };

  async function offerPendingSave() {
    const pending = (await call({ kind: "pending" })) as PendingPrompt | null | undefined;
    if (!pending) return;
    showSavePrompt(doc, pending, {
      onSave: () => void call({ kind: "save-pending" }),
      onDismiss: () => void call({ kind: "dismiss-pending" }),
    });
  }

  const stopWatching = watchSubmissions(doc, (s) => {
    void call({ kind: "submitted", username: s.username, password: s.password, title: doc.title || undefined }).then(
      // Single-page apps do not navigate: offer the prompt right away.
      (status) => (status === "new" || status === "password_changed" ? offerPendingSave() : undefined),
    );
  });

  /** Fills the page's login form; closes the inline menu. */
  function fill(apply: (fields: NonNullable<ReturnType<typeof findLoginFields>>) => boolean): boolean {
    inline.close();
    const fields = findLoginFields(doc);
    return fields ? apply(fields) : false;
  }

  /** Handles a message from the background; returns whether it did something. */
  function onMessage(message: TabMessage): boolean {
    // A fill clicked in the inline menu counts only if the menu was really visible.
    if ("fromMenu" in message && message.fromMenu && !inline.menuTrusted()) {
      inline.close();
      return false;
    }
    switch (message.kind) {
      case "fill":
        return fill((fields) => fillLogin(fields, message));
      case "fill-new-password":
        return fill((fields) => fillNewPassword(fields, message.password));
      case "suggest-at-target":
        inline.close();
        return contextTarget?.isConnected ? fillSuggested(contextTarget, message.password) : false;
      case "fill-profile": {
        inline.close();
        const found = findProfileFields(doc);
        const filled = fillProfile(found[message.profile], message.values);
        const own = message.profile === "identity" ? fillOwnFields(doc, message.values, [...found.identity, ...found.card]) : 0;
        return filled + own > 0;
      }
      case "close-menu":
        inline.close();
        return true;
      case "open-menu":
        return inline.openWithKeyboard();
      case "open-passkey":
        inline.openPasskey();
        return true;
      case "passkey-fields":
        inline.setPasskeyFields(message.active);
        return true;
      case "resize-menu":
        inline.resize(message.height);
        return true;
      default:
        return false;
    }
  }

  void offerPendingSave();
  return {
    onMessage,
    stop() {
      stopWatching();
      inline.destroy();
      doc.removeEventListener("contextmenu", onContextMenu, true);
    },
  };
}
