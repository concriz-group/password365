/**
 * Whether the page hides an element from the user. Values are only ever put
 * into fields the user can see: a page can hide extra boxes (CSS, off-screen,
 * 1-pixel) to collect a card or address the user never meant to give it.
 */
export function hiddenFromUser(element: Element): boolean {
  if (element instanceof HTMLInputElement && element.type === "hidden") return true;
  const win = element.ownerDocument.defaultView;
  if (!win) return true;
  for (let node: Element | null = element; node; node = node.parentElement) {
    if (node instanceof HTMLElement && node.hidden) return true;
    const style = win.getComputedStyle(node);
    if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") return true;
    if (Number.parseFloat(style.opacity) < 0.1) return true;
    if (/^rect\(\s*0(px)?[\s,]+0(px)?[\s,]+0(px)?[\s,]+0(px)?\s*\)$/.test(style.clip)) return true;
  }
  // Size and position (only where the browser lays the page out).
  if (element.getClientRects().length > 0) {
    const r = element.getBoundingClientRect();
    if (r.width < 4 || r.height < 4) return true;
    if (r.right + win.scrollX <= 0 || r.bottom + win.scrollY <= 0) return true;
  }
  return false;
}
