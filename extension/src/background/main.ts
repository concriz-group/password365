// Background service worker entry point: wires chrome.* APIs into the handler.
import type { Reply, TabMessage } from "../shared/messages";
import { DEFAULT_GENERATE, type Generated, HOST_NAME, Password365Error } from "../shared/protocol";
import { Bridge } from "./bridge";
import { CONTEXT_MENUS, handleContextMenuClick } from "./contextMenu";
import { handleShortcut } from "./shortcut";
import { createHandler, type PromptStore, type StoredPrompt } from "./handlers";

const bridge = new Bridge(
  () => chrome.runtime.connectNative(HOST_NAME),
  () => chrome.runtime.lastError?.message,
);

// chrome.storage.session lives in memory only and is not readable by content scripts.
const prompts: PromptStore = {
  async get(tabId) {
    const key = `prompt:${tabId}`;
    return (await chrome.storage.session.get(key))[key] as StoredPrompt | undefined;
  },
  set: (tabId, prompt) => chrome.storage.session.set({ [`prompt:${tabId}`]: prompt }),
  delete: (tabId) => chrome.storage.session.remove(`prompt:${tabId}`),
};

const openPopup = () => chrome.action.openPopup();
const sendToTab = async (tabId: number, message: TabMessage) => {
  await chrome.tabs.sendMessage(tabId, message, { frameId: 0 });
};

const tabsApi = {
  url: async (tabId: number) => (await chrome.tabs.get(tabId)).url,
  send: sendToTab,
};

const handle = createHandler({
  extensionId: chrome.runtime.id,
  extensionOrigin: chrome.runtime.getURL("/"),
  bridge,
  prompts,
  openPopup,
  tabs: {
    url: async (tabId) => (await chrome.tabs.get(tabId)).url,
    send: sendToTab,
  },
});

chrome.runtime.onMessage.addListener((message, sender, sendResponse: (reply: Reply) => void) => {
  handle(message, sender).then(
    (result) => sendResponse({ ok: true, result }),
    (e: unknown) => {
      const error = e instanceof Password365Error ? e : new Password365Error("internal", String(e));
      sendResponse({ ok: false, error: { code: error.code, message: error.message } });
    },
  );
  return true; // async response
});

chrome.tabs.onRemoved.addListener((tabId) => void prompts.delete(tabId));

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.removeAll(() => {
    for (const item of CONTEXT_MENUS) chrome.contextMenus.create(item);
  });
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  void handleContextMenuClick(info, tab?.id, {
    generate: () => bridge.request<Generated>({ type: "generate", request: DEFAULT_GENERATE }),
    send: sendToTab,
    openPopup,
  });
});

chrome.commands.onCommand.addListener((command, tab) => {
  void handleShortcut(command, tab?.id, { bridge, tabs: tabsApi, openPopup });
});
