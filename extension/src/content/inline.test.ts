import { BADGE_ID_PREFIX, createInline, MENU_ARMING_MS, MENU_HOST_ID, MENU_WIDTH } from "./inline";

const menuSrc = (mode: string) => new RegExp(`^chrome-extension://abc/menu\\.html\\?mode=${mode}&t=[0-9a-f]{32}$`);
const OPTS = { menuUrl: "chrome-extension://abc/menu.html", iconUrl: "chrome-extension://abc/icons/32.png" };

function page(html: string) {
  document.body.innerHTML = html;
  return document;
}
const menuHost = () => document.getElementById(MENU_HOST_ID);
const badgeHosts = () => document.querySelectorAll(`[id^=${BADGE_ID_PREFIX}]`);
const rect = (left: number, top: number, width: number, height: number) =>
  ({ left, top, width, height, right: left + width, bottom: top + height }) as DOMRect;

describe("createInline", () => {
  it("puts an icon in the username and password fields", () => {
    page(`<form><input name="user"><input type="password"></form>`);
    const inline = createInline(document, OPTS);
    expect(inline.anchors().map((f) => f.name || f.type)).toEqual(["user", "password"]);
    expect(badgeHosts()).toHaveLength(2);
    inline.destroy();
    expect(badgeHosts()).toHaveLength(0);
  });

  it("opens the menu iframe under a focused field and closes on Escape", () => {
    page(`<form><input name="user"><input type="password"></form>`);
    const inline = createInline(document, OPTS);
    const user = document.querySelector<HTMLInputElement>("input[name=user]")!;
    user.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    expect(inline.anchor).toBe(user);
    expect(menuHost()).not.toBeNull();
    expect(menuHost()!.shadowRoot).toBeNull(); // closed: the page can't reach the iframe
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    expect(menuHost()).not.toBeNull();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(menuHost()).toBeNull();
    inline.destroy();
  });

  it("asks the menu for sign-up mode on sign-up forms", () => {
    page(`<form><input name="user"><input type="password"><input type="password"></form>`);
    const createElement = vi.spyOn(document, "createElement");
    const inline = createInline(document, OPTS);
    inline.open(inline.anchors()[1]!);
    const frame = createElement.mock.results.map((r) => r.value).find((e) => e instanceof HTMLIFrameElement);
    expect((frame as HTMLIFrameElement).src).toMatch(menuSrc('signup'));
    createElement.mockRestore();
    inline.destroy();
  });

  it("shows the passkey prompt at the top right until it is answered", () => {
    page(`<p>Sign in</p><form><input name="user"><input type="password"></form>`);
    const createElement = vi.spyOn(document, "createElement");
    const inline = createInline(document, OPTS);
    inline.openPasskey();
    const frame = createElement.mock.results.map((r) => r.value).find((e) => e instanceof HTMLIFrameElement);
    expect((frame as HTMLIFrameElement).src).toMatch(menuSrc('passkey'));
    expect(inline.anchor).toBeUndefined();
    document.querySelector("p")!.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(menuHost(), "a click on the page does not dismiss it").not.toBeNull();
    window.dispatchEvent(new Event("scroll"));
    inline.close();
    expect(menuHost()).toBeNull();
    createElement.mockRestore();
    inline.destroy();
  });

  it("adds the menu to the site's passkey field while passkeys are offered", () => {
    page(`<form><input name="handle" autocomplete="username webauthn"></form>`);
    const inline = createInline(document, OPTS);
    expect(inline.anchors()).toHaveLength(0);
    inline.setPasskeyFields(true);
    expect(inline.anchors().map((f) => f.name)).toEqual(["handle"]);
    inline.setPasskeyFields(false);
    expect(inline.anchors()).toHaveLength(0);
    inline.destroy();
  });

  it("puts icons in address and payment fields and opens the matching menu", () => {
    page(`<form><input autocomplete="cc-number"><input autocomplete="cc-csc"></form>
          <form><input autocomplete="given-name"><input autocomplete="postal-code"></form>`);
    const createElement = vi.spyOn(document, "createElement");
    const inline = createInline(document, OPTS);
    expect(inline.anchors()).toHaveLength(4);
    expect(inline.profileFields.card).toHaveLength(2);
    const frames = () =>
      createElement.mock.results.map((r) => r.value).filter((e): e is HTMLIFrameElement => e instanceof HTMLIFrameElement);
    inline.open(document.querySelector<HTMLInputElement>("[autocomplete=cc-csc]")!);
    expect(frames().at(-1)!.src).toMatch(menuSrc('card'));
    inline.open(document.querySelector<HTMLInputElement>("[autocomplete=given-name]")!);
    expect(frames().at(-1)!.src).toMatch(menuSrc('identity'));
    createElement.mockRestore();
    inline.destroy();
  });

  it("closes on outside clicks and typing, but not on clicks in the field or on fills", () => {
    page(`<form><input name="user"><input type="password"></form><p id="out">x</p>`);
    const inline = createInline(document, OPTS);
    const [user] = inline.anchors();
    inline.open(user!);
    inline.open(user!); // already open: no-op
    user!.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    user!.dispatchEvent(new Event("input", { bubbles: true })); // synthetic (a fill): stays open
    expect(menuHost()).not.toBeNull();
    document.getElementById("out")!.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(menuHost()).toBeNull();
    document.getElementById("out")!.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));

    const pass = inline.anchors()[1]!;
    inline.open(pass);
    pass.dispatchEvent(new InputEvent("input", { bubbles: true }));
    expect(menuHost()).toBeNull();
    inline.destroy();
  });

  it("toggles from the icon, which keeps focus in the field", () => {
    page(`<input name="user"><input type="password">`);
    const createElement = vi.spyOn(document, "createElement");
    const inline = createInline(document, OPTS);
    const button = createElement.mock.results.map((r) => r.value).find((e) => e instanceof HTMLButtonElement)!;
    createElement.mockRestore();
    const down = new MouseEvent("mousedown", { bubbles: true, cancelable: true });
    button.dispatchEvent(down);
    expect(down.defaultPrevented).toBe(true);
    button.click();
    expect(inline.anchor).toBe(inline.anchors()[0]);
    button.click();
    expect(inline.anchor).toBeUndefined();
    inline.destroy();
  });

  it("positions icons inside fields and the menu under the field; hides icons of hidden fields", () => {
    page(`<input name="user"><input type="password">`);
    const [user, pass] = [...document.querySelectorAll("input")];
    user!.getBoundingClientRect = () => rect(10, 20, 200, 30);
    const inline = createInline(document, OPTS);
    const [userBadge, passBadge] = [...badgeHosts()] as HTMLElement[];
    expect(userBadge!.style.left).toBe(`${210 - 20 - 8}px`);
    expect(userBadge!.style.top).toBe("25px");
    expect(userBadge!.style.display).toBe("");
    expect(passBadge!.style.display).toBe("none");
    inline.open(user!);
    expect(menuHost()!.style.left).toBe("10px");
    expect(menuHost()!.style.top).toBe("54px");
    pass!.getBoundingClientRect = () => rect(0, 0, 100, 10);
    window.dispatchEvent(new Event("resize"));
    expect(passBadge!.style.display).toBe("");
    expect(MENU_WIDTH).toBeGreaterThan(200);
    inline.destroy();
  });

  it("resizes the menu within limits", () => {
    page(`<input name="user"><input type="password">`);
    const createElement = vi.spyOn(document, "createElement");
    const inline = createInline(document, OPTS);
    inline.resize(100); // no menu: ignored
    inline.open(inline.anchors()[0]!);
    const frame = createElement.mock.results.map((r) => r.value).find((e) => e instanceof HTMLIFrameElement)!;
    createElement.mockRestore();
    inline.resize(180.4);
    expect(frame.style.height).toBe("180px");
    inline.resize(5000);
    expect(frame.style.height).toBe("400px");
    inline.resize(Number.NaN);
    expect(frame.style.height).toBe("40px");
    inline.destroy();
  });

  it("finds login forms added later (single-page apps) and drops removed ones", async () => {
    const settle = () => new Promise((r) => setTimeout(r, 300));
    page(`<p>loading</p>`);
    const inline = createInline(document, OPTS);
    expect(inline.anchors()).toHaveLength(0);
    document.body.innerHTML = `<form><input name="user"><input type="password"></form>`;
    await settle();
    expect(inline.anchors()).toHaveLength(2);
    inline.open(inline.anchors()[0]!);
    inline.refresh(); // same fields: menu stays open
    expect(menuHost()).not.toBeNull();
    document.body.innerHTML = `<p>signed in</p>`;
    await settle();
    expect(inline.anchors()).toHaveLength(0);
    expect(menuHost()).toBeNull();
    inline.destroy();
  });

  it("filters the menu while typing the username instead of closing it", () => {
    page(`<form><input name="user"><input type="password"></form>`);
    const createElement = vi.spyOn(document, "createElement");
    const inline = createInline(document, OPTS);
    const user = inline.anchors()[0]!;
    inline.open(user);
    const frame = createElement.mock.results.map((r) => r.value).find((e) => e instanceof HTMLIFrameElement)!;
    createElement.mockRestore();
    // jsdom gives the frame a window; capture what we post to it.
    const post = vi.fn();
    Object.defineProperty(frame, "contentWindow", { value: { postMessage: post } });
    user.value = "oct";
    user.dispatchEvent(new InputEvent("input", { bubbles: true }));
    expect(menuHost()).not.toBeNull();
    expect(post).toHaveBeenCalledWith({ password365Filter: "oct" }, "chrome-extension://abc");
    inline.destroy();
  });

  it("moves focus into the menu with ↓ and back to the field on close, without reopening", () => {
    page(`<form><input name="user"><input type="password"></form><input id="other">`);
    const inline = createInline(document, OPTS);
    const user = inline.anchors()[0]!;
    const down = () => {
      const e = new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true });
      user.dispatchEvent(e);
      return e;
    };
    document.getElementById("other")!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(inline.anchor).toBeUndefined();
    expect(down().defaultPrevented).toBe(true); // opens + focuses the menu
    expect(inline.anchor).toBe(user);
    expect(document.activeElement).toBe(menuHost());
    down(); // already open on this field
    const focusSpy = vi.spyOn(user, "focus");
    inline.close();
    expect(focusSpy).toHaveBeenCalled();
    expect(menuHost()).toBeNull(); // the refocus did not reopen it
    inline.close(); // nothing open: no-op
    inline.destroy();
  });
});

describe("menu protection", () => {
  afterEach(() => vi.useRealTimers());

  it("loads the menu only after the background knows its token", async () => {
    page(`<input name="user"><input type="password">`);
    let release!: () => void;
    const registerMenu = vi.fn(() => new Promise<void>((r) => (release = r)));
    const createElement = vi.spyOn(document, "createElement");
    const inline = createInline(document, { ...OPTS, registerMenu });
    inline.open(inline.anchors()[0]!);
    const frame = createElement.mock.results.map((r) => r.value).find((e) => e instanceof HTMLIFrameElement)!;
    createElement.mockRestore();
    expect(frame.getAttribute("src")).toBeNull();
    const token = (registerMenu.mock.calls[0] as unknown as [string])[0];
    expect(token).toMatch(/^[0-9a-f]{32}$/);
    release();
    await new Promise((r) => setTimeout(r, 0));
    expect(frame.src).toBe(`${OPTS.menuUrl}?mode=login&t=${token}`);
    inline.destroy();
  });

  it("trusts a click in the menu only once it has been visible for a moment, uncovered", () => {
    vi.useFakeTimers();
    page(`<input name="user"><input type="password">`);
    const inline = createInline(document, OPTS);
    expect(inline.menuTrusted()).toBe(false); // no menu
    inline.open(inline.anchors()[0]!);
    expect(inline.menuTrusted()).toBe(false); // just opened: a page could pop it under the pointer
    vi.advanceTimersByTime(MENU_ARMING_MS);
    expect(inline.menuTrusted()).toBe(true);
    // The page makes it see-through, or hides it.
    menuHost()!.style.setProperty("opacity", "0");
    expect(inline.menuTrusted()).toBe(false);
    menuHost()!.style.removeProperty("opacity");
    document.documentElement.style.filter = "opacity(0)";
    expect(inline.menuTrusted()).toBe(false);
    document.documentElement.style.filter = "";
    menuHost()!.style.visibility = "hidden";
    expect(inline.menuTrusted()).toBe(false);
    menuHost()!.style.visibility = "";
    expect(inline.menuTrusted()).toBe(true);
    inline.destroy();
  });

  it("refuses a menu covered by something of the page's", () => {
    vi.useFakeTimers();
    page(`<input name="user"><input type="password"><div id="cover"></div>`);
    const inline = createInline(document, OPTS);
    inline.open(inline.anchors()[0]!);
    vi.advanceTimersByTime(MENU_ARMING_MS);
    const original = HTMLIFrameElement.prototype.getBoundingClientRect;
    HTMLIFrameElement.prototype.getBoundingClientRect = () => rect(10, 50, 320, 120);
    const cover = document.getElementById("cover")!;
    document.elementFromPoint = () => cover;
    expect(inline.menuTrusted()).toBe(false);
    document.elementFromPoint = () => menuHost();
    expect(inline.menuTrusted()).toBe(true);
    HTMLIFrameElement.prototype.getBoundingClientRect = original;
    delete (document as { elementFromPoint?: unknown }).elementFromPoint;
    inline.destroy();
  });
});
