/**
 * The one-time token of this menu, sent by the content script that opened it
 * (a message addressed to the extension's origin, once the page has loaded).
 * It proves to the background that this frame is the menu the content script
 * opened, not a copy of menu.html framed by the web page. It never appears in
 * the frame's address, which the web page can see.
 *
 * Only a message from the parent window counts, and only the first one.
 */
export function receiveToken(win: Window): Promise<string> {
  return new Promise((resolve) => {
    const onMessage = (e: MessageEvent) => {
      const token = (e.data as { password365Token?: unknown } | null)?.password365Token;
      if (e.source !== win.parent || typeof token !== "string" || token === "") return;
      win.removeEventListener("message", onMessage);
      resolve(token);
    };
    win.addEventListener("message", onMessage);
  });
}
