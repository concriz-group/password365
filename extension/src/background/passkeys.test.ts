import { createPasskeys } from "./passkeys";

const createOptions = { rp_id: "example.com", rp_name: "Example", user_id: "AQ", user_name: "ana", challenge: "Ag", algorithms: [-7], exclude: ["Bw"] };
const getOptions = { rp_id: "example.com", challenge: "Aw", allow: [] };
const choice = { item_id: "i1", title: "Example", user_name: "ana" };

function setup({ unlocked = true, initialized = true, choices = [choice], url = "https://example.com/a" } = {}) {
  const state = { unlocked };
  const bridge = {
    request: vi.fn(async (body: { type: string }) => {
      switch (body.type) {
        case "status":
          return { initialized, unlocked: state.unlocked };
        case "passkey_options":
          return choices;
        case "passkey_register":
          return { item_id: "new", registration: { credential_id: "AQ" } };
        case "passkey_assert":
          return { credential_id: "AQ", signature: "sig" };
      }
      throw new Error(body.type);
    }),
  };
  const tabs = { url: vi.fn().mockResolvedValue(url), send: vi.fn().mockResolvedValue(undefined) };
  const openPopup = vi.fn().mockResolvedValue(undefined);
  const passkeys = createPasskeys({ bridge: bridge as never, tabs, openPopup, timeoutMs: 1000 });
  return { bridge, tabs, passkeys, state, openPopup };
}

const flush = () => new Promise((r) => setTimeout(r));

describe("background passkeys", () => {
  it("leaves pages to the browser without an account or a passkey for the site", async () => {
    expect(await setup({ initialized: false }).passkeys.request(1, "https://example.com", "create", createOptions)).toEqual({ fallback: true });
    expect(await setup({ choices: [] }).passkeys.request(1, "https://example.com", "get", getOptions)).toEqual({ fallback: true });
  });

  it("saves a passkey once the user confirms in the extension's frame", async () => {
    const { passkeys, tabs, bridge } = setup();
    const reply = passkeys.request(7, "https://example.com/a", "create", createOptions);
    await flush();
    expect(tabs.send).toHaveBeenCalledWith(7, { kind: "open-passkey" });
    expect(await passkeys.prompt(7)).toEqual({ kind: "create", site: "example.com", user_name: "ana" });
    expect(await passkeys.choose(7)).toBe(true);
    expect(await reply).toEqual({ result: { credential_id: "AQ" } });
    expect(bridge.request).toHaveBeenCalledWith({
      type: "passkey_register",
      page_url: "https://example.com/a",
      title: "Example",
      request: { rp_id: "example.com", user_id: "AQ", user_name: "ana", challenge: "Ag", algorithms: [-7] },
      exclude: ["Bw"],
    });
    expect(tabs.send).toHaveBeenLastCalledWith(7, { kind: "close-menu" });
    expect(await passkeys.prompt(7)).toBeNull();
    expect(await passkeys.choose(7)).toBe(false);
  });

  it("signs in only with one of the offered logins", async () => {
    const { passkeys, bridge } = setup();
    const reply = passkeys.request(2, "https://example.com/a", "get", getOptions);
    await flush();
    expect(await passkeys.prompt(2)).toEqual({ kind: "get", site: "example.com", choices: [choice] });
    expect(await passkeys.choose(2, "someone-else")).toBe(false);
    expect(await passkeys.choose(2, "i1")).toBe(true);
    expect(await reply).toEqual({ result: { credential_id: "AQ", signature: "sig" } });
    expect(bridge.request).toHaveBeenCalledWith({ type: "passkey_assert", item_id: "i1", page_url: "https://example.com/a", challenge: "Aw" });
  });

  it("offers a conditional request's passkeys in the menu until one is picked or the page gives up", async () => {
    const { passkeys, tabs, bridge } = setup();
    const reply = passkeys.conditional(5, "https://example.com/login", getOptions);
    await flush();
    expect(tabs.send).toHaveBeenCalledWith(5, { kind: "passkey-fields", active: true });
    expect(passkeys.offeredChoices(5)).toEqual([choice]);
    expect(await passkeys.autofill(5, "nope")).toBe(false);
    expect(await passkeys.autofill(5, "i1")).toBe(true);
    expect(await reply).toMatchObject({ result: { credential_id: "AQ" } });
    expect(bridge.request).toHaveBeenCalledWith({ type: "passkey_assert", item_id: "i1", page_url: "https://example.com/login", challenge: "Aw" });
    expect(tabs.send).toHaveBeenCalledWith(5, { kind: "passkey-fields", active: false });
    expect(passkeys.offeredChoices(5)).toBeUndefined();

    const first = passkeys.conditional(5, "https://example.com", getOptions);
    await flush();
    void passkeys.conditional(5, "https://example.com", getOptions);
    expect(await first).toEqual({ fallback: true });
    await flush();
    passkeys.abort(5);
    expect(passkeys.offeredChoices(5)).toBeUndefined();
    passkeys.abort(5);
  });

  it("leaves conditional requests to the browser when locked, without passkeys, or without the page script", async () => {
    expect(await setup({ unlocked: false }).passkeys.conditional(1, "https://example.com", getOptions)).toEqual({ fallback: true });
    expect(await setup({ choices: [] }).passkeys.conditional(1, "https://example.com", getOptions)).toEqual({ fallback: true });
    const { passkeys, tabs } = setup();
    tabs.send.mockRejectedValueOnce(new Error("no content script"));
    expect(await passkeys.conditional(1, "https://example.com", getOptions)).toEqual({ fallback: true });
  });

  it("asks to unlock first, then carries on with the request", async () => {
    const { passkeys, state, openPopup } = setup({ unlocked: false });
    const reply = passkeys.request(8, "https://example.com/a", "get", getOptions);
    await flush();
    expect(await passkeys.prompt(8)).toEqual({ kind: "locked", site: "example.com" });
    expect(await passkeys.unlock()).toBe(true);
    expect(openPopup).toHaveBeenCalled();
    state.unlocked = true;
    expect(await passkeys.prompt(8)).toEqual({ kind: "get", site: "example.com", choices: [choice] });
    expect(await passkeys.choose(8, "i1")).toBe(true);
    expect(await reply).toMatchObject({ result: { credential_id: "AQ" } });
  });

  it("after unlocking, a vault without a passkey for the site hands over to the browser", async () => {
    const { passkeys, state, tabs } = setup({ unlocked: false, choices: [] });
    const reply = passkeys.request(9, "https://example.com/a", "get", getOptions);
    await flush();
    state.unlocked = true;
    expect(await passkeys.prompt(9)).toBeNull();
    expect(await reply).toEqual({ fallback: true });
    expect(tabs.send).toHaveBeenLastCalledWith(9, { kind: "close-menu" });
    expect(await passkeys.choose(9, "i1")).toBe(false);
  });

  it("hands over to the browser, refuses on close or timeout, and when the tab moved on", async () => {
    vi.useFakeTimers();
    try {
      const { passkeys, tabs } = setup();
      const other = passkeys.request(3, "https://example.com", "create", createOptions);
      await vi.advanceTimersByTimeAsync(0);
      await passkeys.other(3);
      expect(await other).toEqual({ fallback: true });

      const closed = passkeys.request(3, "https://example.com", "create", createOptions);
      await vi.advanceTimersByTimeAsync(0);
      passkeys.cancel(3);
      expect(await closed).toMatchObject({ error: { name: "NotAllowedError" } });

      const late = passkeys.request(3, "https://example.com", "create", createOptions);
      await vi.advanceTimersByTimeAsync(1000);
      expect(await late).toMatchObject({ error: { name: "NotAllowedError" } });

      const first = passkeys.request(3, "https://example.com", "create", createOptions);
      await vi.advanceTimersByTimeAsync(0);
      const second = passkeys.request(3, "https://example.com", "create", createOptions);
      await vi.advanceTimersByTimeAsync(0);
      expect(await first).toEqual({ fallback: true });
      tabs.url.mockResolvedValue("https://evil.com/");
      expect(await passkeys.choose(3)).toBe(false);
      expect(await second).toMatchObject({ error: { name: "NotAllowedError" } });

      // The browser may not reveal the tab's address: the request's own site counts.
      tabs.url.mockResolvedValue(undefined as never);
      const unknown = passkeys.request(5, "https://example.com", "create", createOptions);
      await vi.advanceTimersByTimeAsync(0);
      expect(await passkeys.choose(5)).toBe(true);
      expect(await unknown).toEqual({ result: { credential_id: "AQ" } });

      tabs.send.mockRejectedValueOnce(new Error("no content script"));
      expect(await passkeys.request(4, "https://example.com", "create", createOptions)).toEqual({ fallback: true });
    } finally {
      vi.useRealTimers();
    }
  });
});
