/**
 * An element of ours on top of a web page, rendered in a **closed** shadow
 * root so the page can neither read nor restyle what is inside.
 */
export interface Overlay {
  host: HTMLElement;
  root: ShadowRoot;
  remove: () => void;
}

/** Creates an overlay with `css`, replacing any previous one with the same id. */
export function createOverlay(doc: Document, id: string, css: string): Overlay {
  doc.getElementById(id)?.remove();
  const host = doc.createElement("div");
  host.id = id;
  const root = host.attachShadow({ mode: "closed" });
  const style = doc.createElement("style");
  style.textContent = `:host { all: initial; }\n${css}`;
  root.append(style);
  doc.documentElement.append(host);
  return { host, root, remove: () => host.remove() };
}

/** Places an overlay host at absolute page coordinates. */
export function placeAt(host: HTMLElement, left: number, top: number): void {
  Object.assign(host.style, {
    position: "absolute",
    left: `${Math.round(left)}px`,
    top: `${Math.round(top)}px`,
    zIndex: "2147483647",
  });
}
