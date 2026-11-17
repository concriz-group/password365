import type { StoredPrompt } from "./handlers";
import { createHandler, PROMPT_TTL_MS } from "./handlers";

const EXT = "ext-id";
const MENU_TOKEN = "0123456789abcdef0123456789abcdef";
const POPUP = { id: EXT };
const tab = (url = "https://github.com/login") => ({ id: EXT, url, frameId: 0, tab: { id: 7 } });

function setup(responses: Record<string, unknown> = {}) {
  const request = vi.fn(async (body: { type: string }) => {
    if (body.type in responses) return responses[body.type];
    throw new Error(`unexpected ${body.type}`);
  });
  const store = new Map<number, StoredPrompt>();
  const tabs = { url: vi.fn(async () => "https://github.com/x"), send: vi.fn(async () => {}) };
  const openPopup = vi.fn(async () => {});
  let time = 1_000;
  const menus = new Map<number, string>([[7, MENU_TOKEN]]);
  const raw = createHandler({
    extensionId: EXT,
    extensionOrigin: `chrome-extension://${EXT}/`,
    bridge: { request } as never,
    tabs,
    openPopup,
    prompts: {
      get: async (id) => store.get(id),
      set: async (id, p) => void store.set(id, p),
      delete: async (id) => void store.delete(id),
    },
    menus: {
      get: async (id) => menus.get(id),
      set: async (id, t) => void menus.set(id, t),
      delete: async (id) => void menus.delete(id),
    },
    now: () => time,
  });
  // Menu messages carry the token of the menu the content script opened in tab 7.
  const handle: typeof raw = (message, sender) =>
    raw(String(message?.kind).startsWith("menu-") && !("token" in message) ? { ...message, token: MENU_TOKEN } : message, sender);
  return { handle, raw, menus, request, tabs, openPopup, store, advance: (ms: number) => (time += ms) };
}

describe("security checks", () => {
  it("rejects foreign senders and malformed messages", async () => {
    const { handle } = setup();
    await expect(handle({ kind: "status" }, { id: "other" })).rejects.toMatchObject({ code: "forbidden" });
    await expect(handle({}, POPUP)).rejects.toMatchObject({ code: "forbidden" });
  });

  it("refuses popup actions from web pages and content actions from the popup", async () => {
    const { handle } = setup();
    await expect(handle({ kind: "fill", tabId: 1, itemId: "x" }, tab())).rejects.toMatchObject({
      code: "forbidden",
    });
    await expect(handle({ kind: "pending" }, POPUP)).rejects.toMatchObject({ code: "forbidden" });
  });

  it("only accepts content messages from the top frame with a URL", async () => {
    const { handle } = setup();
    await expect(handle({ kind: "pending" }, { ...tab(), frameId: 3 })).rejects.toMatchObject({ code: "forbidden" });
    await expect(handle({ kind: "pending" }, { ...tab(), url: undefined })).rejects.toMatchObject({
      code: "forbidden",
    });
  });
});

describe("popup actions", () => {
  it("status, logins and generate go to the bridge with the tab's trusted URL", async () => {
    const { handle, request } = setup({ status: { unlocked: true }, logins_for_url: [], generate: { value: "x" } });
    await expect(handle({ kind: "status" }, POPUP)).resolves.toEqual({ unlocked: true });
    await handle({ kind: "logins", tabId: 7 }, POPUP);
    expect(request).toHaveBeenLastCalledWith({ type: "logins_for_url", page_url: "https://github.com/x" });
    await expect(handle({ kind: "generate" }, POPUP)).resolves.toEqual({ value: "x" });
  });

  it("fill sends credentials to the tab", async () => {
    const { handle, tabs } = setup({ fill: { username: "u", password: "p" } });
    await expect(handle({ kind: "fill", tabId: 7, itemId: "i1" }, POPUP)).resolves.toBe(true);
    expect(tabs.send).toHaveBeenCalledWith(7, { kind: "fill", username: "u", password: "p" });
  });

  it("unlock passes the master password to the app", async () => {
    const { handle, request } = setup({ unlock: null });
    await expect(handle({ kind: "unlock", password: "pw" }, POPUP)).resolves.toBe(true);
    expect(request).toHaveBeenCalledWith({ type: "unlock", password: "pw" });
    await expect(handle({ kind: "unlock", password: "pw" }, tab())).rejects.toMatchObject({ code: "forbidden" });
  });

  it("fails without a tab URL", async () => {
    const { handle, tabs } = setup();
    tabs.url.mockResolvedValueOnce(undefined as never);
    await expect(handle({ kind: "logins", tabId: 7 }, POPUP)).rejects.toMatchObject({ code: "no_page" });
  });
});

describe("save prompts", () => {
  it("stores a pending prompt for new logins, using the sender URL", async () => {
    const { handle, request, store } = setup({ check_submitted: { status: "new" } });
    const status = await handle({ kind: "submitted", username: "me", password: "pw", title: "Hub" }, tab());
    expect(status).toBe("new");
    expect(request).toHaveBeenCalledWith({
      type: "check_submitted",
      login: { page_url: "https://github.com/login", username: "me", password: "pw", title: "Hub" },
    });
    expect(store.get(7)?.status).toBe("new");
    await expect(handle({ kind: "pending" }, tab())).resolves.toEqual({ status: "new", username: "me", site: "github.com" });
  });

  it("keeps the prompt to the origin where the login was submitted", async () => {
    const { handle, request, store } = setup({ check_submitted: { status: "new" }, save_submitted: {} });
    await handle({ kind: "submitted", username: "me", password: "pw" }, tab());
    const elsewhere = tab("https://evil.example/");
    await expect(handle({ kind: "pending" }, elsewhere)).resolves.toBeNull();
    await expect(handle({ kind: "save-pending" }, elsewhere)).resolves.toBeNull();
    expect(request).not.toHaveBeenCalledWith(expect.objectContaining({ type: "save_submitted" }));
    expect(store.size).toBe(1);
    await expect(handle({ kind: "save-pending" }, tab("https://github.com/home"))).resolves.toBe(true);
  });

  it("does not prompt for known logins or empty passwords", async () => {
    const { handle, store } = setup({ check_submitted: { status: "known", id: "x" } });
    await handle({ kind: "submitted", password: "pw" }, tab());
    await expect(handle({ kind: "submitted", password: "" }, tab())).resolves.toBeNull();
    expect(store.size).toBe(0);
    await expect(handle({ kind: "pending" }, tab())).resolves.toBeNull();
  });

  it("saves and clears the pending prompt", async () => {
    const { handle, request, store } = setup({ check_submitted: { status: "password_changed", id: "x" }, save_submitted: {} });
    await handle({ kind: "submitted", username: "me", password: "new" }, tab());
    await expect(handle({ kind: "save-pending" }, tab())).resolves.toBe(true);
    expect(request).toHaveBeenLastCalledWith({
      type: "save_submitted",
      login: { page_url: "https://github.com/login", username: "me", password: "new", title: undefined },
    });
    expect(store.size).toBe(0);
    await expect(handle({ kind: "save-pending" }, tab())).resolves.toBeNull();
  });

  it("dismisses and expires prompts", async () => {
    const { handle, store, advance } = setup({ check_submitted: { status: "new" } });
    await handle({ kind: "submitted", password: "pw" }, tab());
    await handle({ kind: "dismiss-pending" }, tab());
    expect(store.size).toBe(0);

    await handle({ kind: "submitted", password: "pw" }, tab());
    advance(PROMPT_TTL_MS + 1);
    await expect(handle({ kind: "pending" }, tab())).resolves.toBeNull();
    expect(store.size).toBe(0);
  });
});

describe("inline menu", () => {
  const menu = (path = "/menu.html", host = EXT) => ({
    id: EXT,
    url: `chrome-extension://${host}${path}`,
    frameId: 4,
    tab: { id: 7 },
  });

  it("stops accepting a menu's token once that menu closed", async () => {
    const { raw, menus } = setup();
    const top = { ...tab(), tab: { id: 7 } };
    // An older menu closing does not end the one open now.
    await expect(raw({ kind: "menu-closed", token: "0123456789abcdef-older" }, top)).resolves.toBe(false);
    expect(menus.get(7)).toBe(MENU_TOKEN);
    await expect(raw({ kind: "menu-closed", token: MENU_TOKEN }, top)).resolves.toBe(true);
    expect(menus.has(7)).toBe(false);
    await expect(raw({ kind: "menu-state", token: MENU_TOKEN }, menu())).rejects.toMatchObject({ code: "forbidden" });
    // The menu page itself cannot say a menu closed or opened: only the top frame's content script.
    await expect(raw({ kind: "menu-closed", token: MENU_TOKEN }, menu())).rejects.toMatchObject({ code: "forbidden" });
  });

  it("only accepts menu messages carrying the token of the menu the content script opened", async () => {
    const { raw, menus } = setup();
    await expect(raw({ kind: "menu-state" }, menu())).rejects.toMatchObject({ code: "forbidden" });
    await expect(raw({ kind: "menu-state", token: "guess" }, menu())).rejects.toMatchObject({ code: "forbidden" });
    // The content script (top frame of tab 7) opens a new menu: only its token works now.
    const fresh = "fedcba9876543210fedcba9876543210";
    await expect(raw({ kind: "menu-opened", token: fresh }, { ...tab(), tab: { id: 7 } })).resolves.toBe(true);
    expect(menus.get(7)).toBe(fresh);
    await expect(raw({ kind: "menu-state", token: MENU_TOKEN }, menu())).rejects.toMatchObject({ code: "forbidden" });
    await expect(raw({ kind: "menu-opened", token: "short" }, tab())).resolves.toBe(false);
    // Another tab has no menu.
    await expect(raw({ kind: "menu-state", token: fresh }, { ...menu(), tab: { id: 8 } })).rejects.toMatchObject({
      code: "forbidden",
    });
  });

  it("only accepts menu messages from this extension's menu page", async () => {
    const { handle } = setup();
    await expect(handle({ kind: "menu-state" }, tab())).rejects.toMatchObject({ code: "forbidden" });
    await expect(handle({ kind: "menu-state" }, menu("/popup.html"))).rejects.toMatchObject({ code: "forbidden" });
    await expect(handle({ kind: "menu-state" }, menu("/menu.html", "other"))).rejects.toMatchObject({
      code: "forbidden",
    });
    await expect(handle({ kind: "pending" }, menu())).rejects.toMatchObject({ code: "forbidden" });
    await expect(handle({ kind: "menu-state" }, { ...menu(), url: "not a url" })).rejects.toMatchObject({
      code: "forbidden",
    });
  });

  it("reports account and lock state before listing logins", async () => {
    const none = setup({ status: { initialized: false, unlocked: false } });
    await expect(none.handle({ kind: "menu-state" }, menu())).resolves.toEqual({ state: "no_account" });
    const locked = setup({ status: { initialized: true, unlocked: false } });
    await expect(locked.handle({ kind: "menu-state" }, menu())).resolves.toEqual({ state: "locked" });
  });

  it("lists logins for the tab's trusted URL", async () => {
    const logins = [{ id: "i1", title: "GitHub", username: "me", match_level: "site" }];
    const { handle, request } = setup({ status: { initialized: true, unlocked: true }, logins_for_url: logins });
    await expect(handle({ kind: "menu-state" }, menu())).resolves.toEqual({ state: "ready", logins, profile: null });
    expect(request).toHaveBeenCalledWith({ type: "logins_for_url", page_url: "https://github.com/x" });
  });

  it("includes the profile and fills it into the tab", async () => {
    const profile = { identity: null, card: { id: "c", title: "Visa", last4: "4242" } };
    const values = { "cc-number": "4111111111114242" };
    const { handle, request, tabs } = setup({
      status: { initialized: true, unlocked: true },
      logins_for_url: [],
      profile,
      profile_fill: values,
    });
    await expect(handle({ kind: "menu-state" }, menu())).resolves.toEqual({ state: "ready", logins: [], profile });
    await expect(handle({ kind: "menu-fill-profile", profile: "card" }, menu())).resolves.toBe(true);
    expect(request).toHaveBeenLastCalledWith({ type: "profile_fill", profile: "card", item_id: null, page_url: "https://github.com/x" });
    expect(tabs.send).toHaveBeenLastCalledWith(7, { kind: "fill-profile", profile: "card", values, fromMenu: true });
    await handle({ kind: "menu-fill-profile", profile: "identity", itemId: "work" }, menu());
    expect(request).toHaveBeenLastCalledWith(expect.objectContaining({ item_id: "work" }));
    await handle({ kind: "menu-fill-profile", profile: "anything else" }, menu());
    expect(request).toHaveBeenLastCalledWith(expect.objectContaining({ profile: "identity" }));
  });

  it("unlock closes the menu and opens the popup (the password is never typed into the page)", async () => {
    const { handle, tabs, openPopup } = setup();
    await expect(handle({ kind: "menu-unlock" }, menu())).resolves.toBe(true);
    expect(tabs.send).toHaveBeenLastCalledWith(7, { kind: "close-menu" });
    expect(openPopup).toHaveBeenCalled();
    openPopup.mockRejectedValueOnce(new Error("no focused window"));
    await expect(handle({ kind: "menu-unlock" }, menu())).rejects.toMatchObject({ code: "open_popup" });
  });

  it("fills the tab, suggests passwords and closes", async () => {
    const { handle, tabs } = setup({ fill: { username: "u", password: "p" }, generate: { value: "Gen3rated!" } });
    await expect(handle({ kind: "menu-fill", itemId: "i1" }, menu())).resolves.toBe(true);
    expect(tabs.send).toHaveBeenLastCalledWith(7, { kind: "fill", username: "u", password: "p", fromMenu: true });
    await expect(handle({ kind: "menu-suggest" }, menu())).resolves.toBe(true);
    expect(tabs.send).toHaveBeenLastCalledWith(7, { kind: "fill-new-password", password: "Gen3rated!", fromMenu: true });
    await expect(handle({ kind: "menu-close" }, menu())).resolves.toBe(true);
    expect(tabs.send).toHaveBeenLastCalledWith(7, { kind: "close-menu" });
    await handle({ kind: "menu-resize", height: 180 }, menu());
    expect(tabs.send).toHaveBeenLastCalledWith(7, { kind: "resize-menu", height: 180 });
    await handle({ kind: "menu-resize", height: "x" }, menu());
    expect(tabs.send).toHaveBeenLastCalledWith(7, { kind: "resize-menu", height: 0 });
  });
});

describe("passkeys", () => {
  const menuFrame = { id: EXT, url: `chrome-extension://${EXT}/menu.html`, frameId: 4, tab: { id: 7 } };

  it("takes the site from the tab, asks in the menu frame and answers the page", async () => {
    const { handle, request, tabs } = setup({
      status: { initialized: true, unlocked: true },
      passkey_options: [{ item_id: "i1", title: "GitHub", user_name: "octo" }],
      passkey_assert: { credential_id: "AQ" },
    });
    tabs.url.mockResolvedValue("https://github.com/login");
    const page = handle({ kind: "passkey-get", options: { rp_id: "github.com", challenge: "Ag", allow: [] } }, tab());
    await new Promise((r) => setTimeout(r));
    expect(request).toHaveBeenCalledWith({ type: "passkey_options", page_url: "https://github.com/login", rp_id: "github.com", allow: [] });
    expect(tabs.send).toHaveBeenCalledWith(7, { kind: "open-passkey" });
    expect(await handle({ kind: "menu-passkey-state" }, menuFrame)).toMatchObject({ kind: "get", site: "github.com" });
    await expect(handle({ kind: "menu-passkey-choose", itemId: "i1" }, tab())).rejects.toMatchObject({ code: "forbidden" });
    expect(await handle({ kind: "menu-passkey-choose", itemId: "i1" }, menuFrame)).toBe(true);
    expect(await page).toEqual({ result: { credential_id: "AQ" } });
  });

  it("closing the prompt refuses; another method hands over to the browser", async () => {
    const { handle } = setup({ status: { initialized: true, unlocked: true } });
    const options = { rp_id: "github.com", rp_name: "GitHub", user_id: "AQ", user_name: "octo", challenge: "Ag", algorithms: [-7], exclude: [] };
    const refused = handle({ kind: "passkey-create", options }, tab());
    await new Promise((r) => setTimeout(r));
    await handle({ kind: "menu-close" }, menuFrame);
    expect(await refused).toMatchObject({ error: { name: "NotAllowedError" } });
    const handed = handle({ kind: "passkey-create", options }, tab());
    await new Promise((r) => setTimeout(r));
    expect(await handle({ kind: "menu-passkey-other" }, menuFrame)).toBe(true);
    expect(await handed).toEqual({ fallback: true });
    expect(await handle({ kind: "menu-passkey-unlock" }, menuFrame)).toBe(true);
    expect(await handle({ kind: "passkey-abort" }, tab())).toBe(true);
    expect(await handle({ kind: "passkey-waiting", tabId: 7 }, POPUP)).toBe(false);
    expect(await handle({ kind: "menu-passkey-autofill", itemId: "x" }, menuFrame)).toBe(false);
  });
});
