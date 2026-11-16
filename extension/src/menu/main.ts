// Inline menu entry point: runs in an extension iframe placed under a login field.
import type { FromMenu, MenuMessage, Reply } from "../shared/messages";
import { applyFilter, type MenuMode, moveFocus, renderMenu } from "./view";

// The one-time token the content script put in our URL: proves this frame is the
// menu it opened, not a copy of menu.html framed by the page.
const token = new URLSearchParams(location.search).get("t") ?? "";
const send = (message: MenuMessage) => chrome.runtime.sendMessage<FromMenu, Reply>({ ...message, token });
const MODES: MenuMode[] = ["login", "signup", "identity", "card", "passkey"];
const asked = new URLSearchParams(location.search).get("mode") as MenuMode;
const mode: MenuMode = MODES.includes(asked) ? asked : "login";
const root = document.getElementById("app")!;

// Tell the content script how tall we are (through the extension, not postMessage).
new ResizeObserver(() => void send({ kind: "menu-resize", height: document.body.scrollHeight })).observe(document.body);
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") void send({ kind: "menu-close" });
  else if (moveFocus(root, e.key)) e.preventDefault();
});
// ↓ in the field focuses this frame (content script): start on the first row.
window.addEventListener("focus", () => moveFocus(root, "ArrowDown"));
// Filter text typed in the username field. The page could post this too, but it
// only changes which of our rows are visible — nothing is read back.
window.addEventListener("message", (e) => {
  const data = e.data as { password365Filter?: unknown } | null;
  if (typeof data?.password365Filter === "string") applyFilter(root, data.password365Filter);
});
void renderMenu(root, { send, mode });
