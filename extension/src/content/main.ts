// Content script entry point (top frame only, see manifest).
import type { ContentMessage, Reply, TabMessage } from "../shared/messages";
import { startContent } from "./controller";

const content = startContent(
  document,
  (message: ContentMessage) => chrome.runtime.sendMessage<ContentMessage, Reply>(message),
  { menuUrl: chrome.runtime.getURL("menu.html"), iconUrl: chrome.runtime.getURL("icons/32.png") },
);

chrome.runtime.onMessage.addListener((message: TabMessage, sender, sendResponse) => {
  if (sender.id !== chrome.runtime.id) return false;
  sendResponse(content.onMessage(message));
  return false;
});
