// Keyboard shortcuts as the backend stores them ("Shift+Super+Space") and as
// people read them ("⇧⌘Space" on a Mac, "Ctrl+Shift+Space" elsewhere).

const MODIFIERS = new Set(["Control", "Alt", "Shift", "Meta", "OS"]);

/** The shortcut a key press makes, or null (a lone modifier, or no modifier on a normal key). */
export function fromKeyEvent(e: Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): string | null {
  if (MODIFIERS.has(e.key)) return null;
  const parts = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean) as string[];
  const functionKey = /^F\d{1,2}$/.test(e.code);
  if (parts.length === 0 && !functionKey) return null;
  return [...parts, e.code].join("+");
}

/** "Shift+Super+KeyL" → "⇧⌘L" on a Mac, "Shift+Win+L" elsewhere. */
export function displayShortcut(shortcut: string, mac: boolean): string {
  if (!shortcut) return "";
  const key = (part: string) => part.replace(/^Key/, "").replace(/^Digit/, "");
  const parts = shortcut.split("+");
  if (mac) {
    const symbol: Record<string, string> = { Ctrl: "⌃", Alt: "⌥", Shift: "⇧", Super: "⌘" };
    const order = ["Ctrl", "Alt", "Shift", "Super"];
    const mods = order.filter((m) => parts.includes(m)).map((m) => symbol[m]).join("");
    return mods + parts.filter((p) => !order.includes(p)).map(key).join("+");
  }
  return parts.map((p) => (p === "Super" ? "Win" : key(p))).join("+");
}
