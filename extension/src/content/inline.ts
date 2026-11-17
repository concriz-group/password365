import { findLoginFields, type LoginFields } from "./detect";
import { type FormField, findProfileFields, type ProfileField, type ProfileKind } from "./profileFields";
import { createOverlay, type Overlay, placeAt } from "./overlay";
import { t } from "../shared/i18n";

export const MENU_HOST_ID = "password365-inline-menu";
export const BADGE_ID_PREFIX = "password365-badge-";
export const MENU_WIDTH = 320;
const BADGE_SIZE = 20;
const MIN_MENU_HEIGHT = 40;
const MAX_MENU_HEIGHT = 400;

const BADGE_CSS = `
  button { all: initial; display: block; width: ${BADGE_SIZE}px; height: ${BADGE_SIZE}px; cursor: pointer;
           border-radius: 50%; }
  button:focus-visible { outline: 2px solid #ff8a1f; }
  img { width: 100%; height: 100%; display: block; border-radius: 50%; }
`;
const MENU_CSS = `
  iframe { display: block; border: 0; width: ${MENU_WIDTH}px; height: 120px; border-radius: 10px;
           box-shadow: 0 8px 30px rgba(0,0,0,.35); background: #1c1c1e; color-scheme: normal; }
`;

export interface InlineOptions {
  /** URL of the extension's `menu.html` (cross-origin to the page). */
  menuUrl: string;
  /** URL of the icon shown inside the fields. */
  iconUrl: string;
  /** Tells the background the token of a menu about to open (the menu proves itself with it). */
  registerMenu?: (token: string) => Promise<unknown>;
  /** Tells the background that the menu with this token closed (the token stops working). */
  unregisterMenu?: (token: string) => unknown;
}

/** How long the menu must have been on screen before a click in it fills anything. */
export const MENU_ARMING_MS = 500;

/** A fresh one-time token for a menu. */
function newToken(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Inline autofill, the way 1Password does it: an icon inside the username and
 * password fields, and a menu under the field when it gets focus (or the icon
 * is clicked). The menu itself is an extension page in an iframe, so the site
 * can't read the saved logins or click on them.
 */
export function createInline(doc: Document, { menuUrl, iconUrl, registerMenu, unregisterMenu }: InlineOptions) {
  const win = doc.defaultView!;
  let login: LoginFields | null = null;
  let profile: Record<ProfileKind, ProfileField[]> = { identity: [], card: [] };
  const badges = new Map<FormField, Overlay>();
  /** The open menu; `anchor` is `null` for the passkey prompt (top right, no field). */
  let menu: { overlay: Overlay; frame: HTMLIFrameElement; anchor: FormField | null; openedAt: number; token: string } | null = null;
  /** Set while we move focus back to the field ourselves, so it doesn't reopen the menu. */
  let refocusing = false;
  // `URL.origin` is "null" for non-web schemes in some runtimes; build it explicitly.
  const menuOrigin = (({ protocol, host }) => `${protocol}//${host}`)(new URL(menuUrl));

  const anchors = () => [...badges.keys()];
  const isOurs = (node: EventTarget | null) =>
    node === menu?.overlay.host || [...badges.values()].some((b) => b.host === node);

  function addBadge(field: FormField, index: number) {
    const overlay = createOverlay(doc, `${BADGE_ID_PREFIX}${index}`, BADGE_CSS);
    const button = doc.createElement("button");
    button.type = "button";
    button.title = "Password365";
    button.setAttribute("aria-label", t("openApp"));
    const img = doc.createElement("img");
    img.src = iconUrl;
    img.alt = "";
    button.append(img);
    // Keep focus in the field, like 1Password's icon.
    button.addEventListener("mousedown", (e) => e.preventDefault());
    button.addEventListener("click", () => (menu?.anchor === field ? close() : open(field)));
    overlay.root.append(button);
    badges.set(field, overlay);
  }

  /** A conditional passkey request waits: the site's passkey fields get the menu too. */
  let passkeyFields = false;

  /** Re-scans the page for a login form and puts icons into its fields. */
  function refresh() {
    login = findLoginFields(doc);
    profile = findProfileFields(doc);
    const wanted: FormField[] = [
      ...(login ? [login.username, ...login.passwords].filter((f): f is HTMLInputElement => f !== null) : []),
      ...[...profile.identity, ...profile.card].map((f) => f.element),
    ];
    if (passkeyFields) {
      // `autocomplete="username webauthn"`: where the site offers passkeys.
      for (const field of doc.querySelectorAll<HTMLInputElement>("input")) {
        const tokens = (field.getAttribute("autocomplete") ?? "").toLowerCase().split(/\s+/);
        if (tokens.includes("webauthn") && !wanted.includes(field)) wanted.push(field);
      }
    }
    const same = wanted.length === badges.size && wanted.every((f) => badges.has(f));
    if (!same) {
      close();
      for (const badge of badges.values()) badge.remove();
      badges.clear();
      wanted.forEach(addBadge);
    }
    reposition();
  }

  function reposition() {
    for (const [field, badge] of badges) {
      const rect = field.getBoundingClientRect();
      const visible = field.isConnected && (rect.width > 0 || rect.height > 0);
      badge.host.style.display = visible ? "" : "none";
      placeAt(
        badge.host,
        rect.right + win.scrollX - BADGE_SIZE - 8,
        rect.top + win.scrollY + (rect.height - BADGE_SIZE) / 2,
      );
    }
    if (menu?.anchor) {
      const rect = menu.anchor.getBoundingClientRect();
      placeAt(menu.overlay.host, rect.left + win.scrollX, rect.bottom + win.scrollY + 4);
    } else if (menu) {
      placeAt(menu.overlay.host, win.scrollX + Math.max(16, win.innerWidth - MENU_WIDTH - 16), win.scrollY + 16);
    }
  }

  /** Which form the field belongs to: an address or payment form, or a login. */
  function profileKind(field: FormField): ProfileKind | null {
    if (profile.card.some((f) => f.element === field)) return "card";
    if (profile.identity.some((f) => f.element === field)) return "identity";
    return null;
  }

  /**
   * Loads the menu page in `frame` once the background knows its token, then
   * hands the token to the page. Never in the URL: the web page can see the
   * addresses of the frames it holds (resource timing) and could then open
   * its own copy of the menu with it. A message addressed to the extension's
   * origin reaches only our menu page, whatever the frame shows by then.
   */
  function load(frame: HTMLIFrameElement, mode: string, token: string) {
    const src = `${menuUrl}?mode=${mode}`;
    frame.addEventListener("load", () => {
      frame.contentWindow?.postMessage({ password365Token: token }, menuOrigin);
    });
    if (!registerMenu) {
      frame.src = src;
      return;
    }
    void registerMenu(token).then(() => {
      if (menu?.frame === frame) frame.src = src;
    });
  }

  function open(anchor: FormField) {
    if (menu?.anchor === anchor) return;
    close();
    const overlay = createOverlay(doc, MENU_HOST_ID, MENU_CSS);
    const frame = doc.createElement("iframe");
    const mode = profileKind(anchor) ?? (login?.isSignup ? "signup" : "login");
    frame.title = "Password365";
    overlay.root.append(frame);
    const token = newToken();
    menu = { overlay, frame, anchor, openedAt: Date.now(), token };
    load(frame, mode, token);
    reposition();
  }

  /**
   * Whether a click in the menu was really the user's: the menu has been on
   * screen for a moment, nothing on the page makes it see-through or covers
   * it. Otherwise a page could hide or cover it and trick a click into it.
   */
  function menuTrusted(): boolean {
    if (!menu || !menu.overlay.host.isConnected) return false;
    if (Date.now() - menu.openedAt < MENU_ARMING_MS) return false;
    for (const node of [menu.overlay.host, doc.documentElement]) {
      const style = win.getComputedStyle(node);
      if (style.display === "none" || style.visibility !== "visible") return false;
      if (style.opacity !== "1" || (style.filter && style.filter !== "none")) return false;
    }
    const rect = menu.frame.getBoundingClientRect();
    if (typeof doc.elementFromPoint === "function" && rect.width > 0 && rect.height > 0) {
      // Something of the page's on top of our menu: refuse.
      const top = doc.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
      if (top !== menu.overlay.host) return false;
    }
    return true;
  }

  function close() {
    if (!menu) return;
    const { overlay, anchor, token } = menu;
    // A closed menu's token is worth nothing any more.
    unregisterMenu?.(token);
    // Keyboard users who were inside the menu go back to the field (like 1Password).
    const focusWasInMenu = doc.activeElement === overlay.host;
    overlay.remove();
    menu = null;
    if (focusWasInMenu && anchor) {
      refocusing = true;
      anchor.focus();
      refocusing = false;
    }
  }

  /** The passkey prompt: the menu page in passkey mode, at the top right of the page. */
  function openPasskey() {
    close();
    const overlay = createOverlay(doc, MENU_HOST_ID, MENU_CSS);
    const frame = doc.createElement("iframe");
    frame.title = "Password365";
    overlay.root.append(frame);
    const token = newToken();
    menu = { overlay, frame, anchor: null, openedAt: Date.now(), token };
    load(frame, "passkey", token);
    reposition();
    frame.focus();
  }

  /** Opens the menu on the login form's first field with keyboard focus (fill shortcut). */
  function openWithKeyboard(): boolean {
    const field = login?.username ?? login?.password;
    if (!field) return false;
    open(field);
    menu!.frame.focus();
    return true;
  }

  /** Sets the menu height (asked by the menu page once it has rendered). */
  function resize(height: number) {
    const clamped = Math.min(MAX_MENU_HEIGHT, Math.max(MIN_MENU_HEIGHT, Math.round(height) || MIN_MENU_HEIGHT));
    if (menu) menu.frame.style.height = `${clamped}px`;
  }

  const onFocus = (e: FocusEvent) => {
    const field = e.target as FormField;
    if (!refocusing && badges.has(field)) open(field);
  };
  const onPointer = (e: Event) => {
    // The passkey prompt stays until answered (the page is waiting for it).
    if (menu?.anchor && e.target !== menu.anchor && !isOurs(e.target)) close();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") return close();
    const field = e.target as FormField;
    if (e.key !== "ArrowDown" || !badges.has(field)) return;
    // ↓ from the field moves into the menu (the menu focuses its first row).
    if (menu?.anchor !== field) open(field);
    e.preventDefault();
    menu!.frame.focus();
  };
  const onInput = (e: Event) => {
    // Typing fires InputEvents; our own fills fire plain Events and keep the menu.
    if (!menu || e.target !== menu.anchor || !(e instanceof win.InputEvent)) return;
    if (menu.anchor && menu.anchor === login?.username) {
      // Filter the list by what is typed, like 1Password.
      menu.frame.contentWindow?.postMessage({ password365Filter: login.username.value }, menuOrigin);
    } else {
      close();
    }
  };
  let pending: ReturnType<typeof setTimeout> | undefined;
  const observer = new win.MutationObserver(() => {
    clearTimeout(pending);
    pending = setTimeout(refresh, 250);
  });

  doc.addEventListener("focusin", onFocus, true);
  doc.addEventListener("mousedown", onPointer, true);
  doc.addEventListener("keydown", onKey, true);
  doc.addEventListener("input", onInput, true);
  win.addEventListener("scroll", reposition, true);
  win.addEventListener("resize", reposition);
  observer.observe(doc.documentElement, { childList: true, subtree: true });
  refresh();

  return {
    refresh,
    open,
    openPasskey,
    /** Shows (or stops showing) the menu on the site's passkey fields. */
    setPasskeyFields(active: boolean) {
      passkeyFields = active;
      refresh();
    },
    openWithKeyboard,
    menuTrusted,
    close,
    resize,
    /** Field the menu is open on, if any. */
    get anchor() {
      return menu?.anchor ?? undefined;
    },
    /** Fields that currently show the icon. */
    anchors,
    /** The address and payment fields found by the last scan. */
    get profileFields() {
      return profile;
    },
    destroy() {
      close();
      for (const badge of badges.values()) badge.remove();
      badges.clear();
      clearTimeout(pending);
      observer.disconnect();
      doc.removeEventListener("focusin", onFocus, true);
      doc.removeEventListener("mousedown", onPointer, true);
      doc.removeEventListener("keydown", onKey, true);
      doc.removeEventListener("input", onInput, true);
      win.removeEventListener("scroll", reposition, true);
      win.removeEventListener("resize", reposition);
    },
  };
}

export type Inline = ReturnType<typeof createInline>;
