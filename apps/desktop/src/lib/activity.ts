/** Events that mean the user is using this window. */
const EVENTS = ["keydown", "pointerdown", "pointermove", "wheel"] as const;
/** At most one report per this many milliseconds. */
export const REPORT_EVERY_MS = 15_000;

/**
 * Tells the backend (`report`) that the user is active in this window, at most
 * every {@link REPORT_EVERY_MS}. Only real input counts for the idle lock:
 * nothing the extension, the SSH agent or a timer does keeps the vault open.
 * Returns a function that stops watching.
 */
export function watchActivity(
  target: Window,
  report: () => void,
  now: () => number = Date.now,
  byUser: (e: Event) => boolean = (e) => e.isTrusted,
): () => void {
  let last = -Infinity;
  const onInput = (e: Event) => {
    if (!byUser(e)) return;
    const t = now();
    if (t - last < REPORT_EVERY_MS) return;
    last = t;
    report();
  };
  for (const name of EVENTS) target.addEventListener(name, onInput, { capture: true, passive: true });
  return () => {
    for (const name of EVENTS) target.removeEventListener(name, onInput, { capture: true });
  };
}
