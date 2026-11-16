import type { AppStatus, LoginCandidate } from "../shared/protocol";
import { createActions, type TabsApi } from "./actions";
import type { Bridge } from "./bridge";

/** Keyboard shortcut (manifest `commands`): ⌘⇧L / Ctrl+Shift+L, like Bitwarden's. */
export const FILL_COMMAND = "fill-login";

interface Deps {
  bridge: Pick<Bridge, "request">;
  tabs: TabsApi;
  openPopup: () => Promise<void>;
}

/**
 * Fill shortcut, 1Password-style: one login for the site → fill it; several
 * (or none) → open the inline menu with keyboard focus; locked → popup.
 */
export async function handleShortcut(command: string, tabId: number | undefined, deps: Deps): Promise<void> {
  if (command !== FILL_COMMAND || tabId === undefined) return;
  const { bridge, tabs, openPopup } = deps;
  const { tabUrl, fillTab } = createActions({ bridge, tabs });
  try {
    const status = await bridge.request<AppStatus>({ type: "status" });
    if (!status.initialized || !status.unlocked) return await openPopup();
    const logins = await bridge.request<LoginCandidate[]>({ type: "logins_for_url", page_url: await tabUrl(tabId) });
    if (logins.length === 1) await fillTab(tabId, logins[0]!.id);
    else await tabs.send(tabId, { kind: "open-menu" });
  } catch {
    // App closed, helper missing, no login form…: the popup explains.
    await openPopup().catch(() => undefined);
  }
}
