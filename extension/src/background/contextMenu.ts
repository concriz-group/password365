import type { TabMessage } from "../shared/messages";
import type { Generated } from "../shared/protocol";
import { t } from "../shared/i18n";

export const PARENT_MENU_ID = "password365";
export const SUGGEST_MENU_ID = "password365-suggest-password";
const PAGES = ["https://*/*", "http://*/*"];

/** Right-click menu on editable fields, like 1Password's: Password365 › Suggest password. */
export const CONTEXT_MENUS: chrome.contextMenus.CreateProperties[] = [
  { id: PARENT_MENU_ID, title: "Password365", contexts: ["editable"], documentUrlPatterns: PAGES },
  { id: SUGGEST_MENU_ID, parentId: PARENT_MENU_ID, title: t("suggestPassword"), contexts: ["editable"], documentUrlPatterns: PAGES },
];

interface Deps {
  generate: () => Promise<Generated>;
  send: (tabId: number, message: TabMessage) => Promise<void>;
  openPopup: () => Promise<void>;
}

/**
 * Handles a click in the browser's own context menu (a real user action the
 * page cannot fake). Top frame only, like every fill.
 */
export async function handleContextMenuClick(
  info: { menuItemId: string | number; frameId?: number },
  tabId: number | undefined,
  { generate, send, openPopup }: Deps,
): Promise<void> {
  if (info.menuItemId !== SUGGEST_MENU_ID || tabId === undefined || info.frameId !== 0) return;
  try {
    const { value } = await generate();
    await send(tabId, { kind: "suggest-at-target", password: value });
  } catch {
    // App closed / helper missing: the popup explains what to do.
    await openPopup().catch(() => undefined);
  }
}
