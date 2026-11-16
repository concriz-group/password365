/** Copied secrets are cleared from the clipboard after this delay. */
export const CLIPBOARD_CLEAR_MS = 90_000;

export interface ClipboardLike {
  writeText(text: string): Promise<void>;
  readText?(): Promise<string>;
}

type Schedule = (fn: () => void, ms: number) => unknown;

/**
 * Copies `text` and clears the clipboard after {@link CLIPBOARD_CLEAR_MS}
 * unless the user copied something else in the meantime.
 */
export async function copySecret(
  text: string,
  clipboard: ClipboardLike = navigator.clipboard,
  schedule: Schedule = setTimeout,
  clearAfterMs: number = CLIPBOARD_CLEAR_MS,
): Promise<void> {
  await clipboard.writeText(text);
  // 0: the user chose to keep copied text (Settings › Security).
  if (clearAfterMs > 0) schedule(() => void clearIfUnchanged(text, clipboard), clearAfterMs);
}

async function clearIfUnchanged(text: string, clipboard: ClipboardLike): Promise<void> {
  try {
    const current = clipboard.readText ? await clipboard.readText() : text;
    if (current === text) await clipboard.writeText("");
  } catch {
    // Reading may be denied: clear anyway, a stale secret is worse than a lost clipboard.
    await clipboard.writeText("");
  }
}
