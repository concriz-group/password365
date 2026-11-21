// Popup entry point.
import type { PopupMessage, Reply } from "../shared/messages";
import { renderPopup } from "./view";

const CLIPBOARD_CLEAR_MS = 90_000;

async function main() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const root = document.getElementById("app");
  if (!root) return;
  await renderPopup(root, {
    tabId: tab?.id,
    send: (message: PopupMessage) => chrome.runtime.sendMessage<PopupMessage, Reply>(message),
    copy: async (text) => {
      await navigator.clipboard.writeText(text);
      // The popup closes quickly, so the clear is best-effort; the desktop app is the safe place to copy from.
      setTimeout(() => void navigator.clipboard.writeText(""), CLIPBOARD_CLEAR_MS);
    },
    close: () => window.close(),
    openUrl: (url) => void chrome.tabs.create({ url }),
  });
}

void main();
