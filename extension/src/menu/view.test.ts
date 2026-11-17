import type { MenuMessage, Reply } from "../shared/messages";
import { renderMenu } from "./view";

const ok = (result: unknown): Reply => ({ ok: true, result });
const fail = (code: string, message = "boom"): Reply => ({ ok: false, error: { code, message } });

function setup(state: Reply, replies: Partial<Record<MenuMessage["kind"], Reply>> = {}) {
  const root = document.createElement("main");
  const send = vi.fn(async (m: MenuMessage) => (m.kind === "menu-state" ? state : (replies[m.kind] ?? ok(true))));
  return { root, send };
}
const flush = () => new Promise((r) => setTimeout(r, 0));
const LOGINS = [
  { id: "i1", title: "GitHub", username: "me", match_level: "site" },
  { id: "i2", title: " ", username: null, match_level: "site" },
];

describe("renderMenu", () => {
  it("explains why it can't fill", async () => {
    for (const [state, text] of [
      [fail("app_not_running"), "Open the Password365 app"],
      [fail("other", "weird"), "weird"],
      [ok({ state: "no_account" }), "Create your account"],
      [ok({ state: "locked" }), "Unlock Password365"],
    ] as const) {
      const { root, send } = setup(state);
      await renderMenu(root, { send, mode: "login" });
      expect(root.textContent).toContain(text);
    }
  });

  it("offers to unlock when locked", async () => {
    const { root, send } = setup(ok({ state: "locked" }), { "menu-unlock": fail("open_popup", "Click the icon") });
    await renderMenu(root, { send, mode: "login" });
    const unlock = root.querySelector<HTMLButtonElement>("button.row")!;
    expect(unlock.textContent).toContain("Unlock Password365");
    unlock.click();
    await flush();
    expect(send).toHaveBeenCalledWith({ kind: "menu-unlock" });
    expect(unlock.textContent).toContain("Click the Password365 icon in the toolbar to unlock.");
  });

  it("lists logins and fills the one clicked", async () => {
    const { root, send } = setup(ok({ state: "ready", logins: LOGINS }));
    await renderMenu(root, { send, mode: "login" });
    const rows = root.querySelectorAll("button.row");
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain("GitHub");
    expect(rows[1]!.querySelector(".avatar")!.textContent).toBe("?");
    (rows[0] as HTMLButtonElement).click();
    expect(send).toHaveBeenLastCalledWith({ kind: "menu-fill", itemId: "i1" });
  });

  it("says when there are no logins, and shows errors in the row", async () => {
    const { root, send } = setup(ok({ state: "ready", logins: null }), { "menu-fill": fail("not_found", "gone") });
    await renderMenu(root, { send, mode: "login" });
    expect(root.textContent).toContain("No saved logins");

    const again = setup(ok({ state: "ready", logins: LOGINS }), { "menu-fill": fail("not_found", "gone") });
    await renderMenu(again.root, { send: again.send, mode: "login" });
    const first = again.root.querySelector<HTMLButtonElement>("button.row")!;
    first.click();
    await flush();
    expect(first.textContent).toContain("gone");
    expect(first.disabled).toBe(false);
  });

  it("offers a suggested password on sign-up forms", async () => {
    const { root, send } = setup(ok({ state: "ready", logins: [] }));
    await renderMenu(root, { send, mode: "signup" });
    expect(root.textContent).not.toContain("No saved logins");
    const suggest = root.querySelector<HTMLButtonElement>("button.row")!;
    expect(suggest.textContent).toContain("Use suggested password");
    suggest.click();
    expect(send).toHaveBeenLastCalledWith({ kind: "menu-suggest" });
  });
});

describe("filter and keyboard", () => {
  async function rendered() {
    const { root, send } = setup(ok({ state: "ready", logins: [
      { id: "a", title: "GitHub", username: "octo", match_level: "site" },
      { id: "b", title: "GitHub work", username: "remo@corp.com", match_level: "site" },
      { id: "c", title: "Gist", username: null, match_level: "site" },
    ] }));
    document.body.replaceChildren(root);
    await renderMenu(root, { send, mode: "login" });
    const visible = () => [...root.querySelectorAll<HTMLButtonElement>("button.row")].filter((b) => !b.hidden);
    return { root, visible };
  }

  it("filters by title or username and says when nothing matches", async () => {
    const { root, visible } = await rendered();
    const { applyFilter } = await import("./view");
    expect(visible()).toHaveLength(3);
    expect(root.querySelector<HTMLElement>(".no-match")!.hidden).toBe(true);
    applyFilter(root, "CORP");
    expect(visible().map((b) => b.textContent)).toEqual(["GGitHub workremo@corp.com"]);
    applyFilter(root, "zzz");
    expect(visible()).toHaveLength(0);
    expect(root.querySelector<HTMLElement>(".no-match")!.hidden).toBe(false);
    applyFilter(root, " ");
    expect(visible()).toHaveLength(3);
  });

  it("moves between visible rows with the arrow keys", async () => {
    const { root, visible } = await rendered();
    const { applyFilter, moveFocus } = await import("./view");
    expect(moveFocus(root, "Enter")).toBe(false);
    moveFocus(root, "ArrowDown");
    expect(document.activeElement).toBe(visible()[0]);
    moveFocus(root, "ArrowUp");
    expect(document.activeElement).toBe(visible()[0]);
    moveFocus(root, "ArrowDown");
    moveFocus(root, "ArrowDown");
    moveFocus(root, "ArrowDown");
    expect(document.activeElement).toBe(visible()[2]);
    applyFilter(root, "nothing");
    expect(moveFocus(root, "ArrowDown")).toBe(false);
  });

  it("offers the chosen card or identity on payment and address forms", async () => {
    const profile = { identity: { id: "me", title: "Me", last4: null }, card: { id: "c", title: "Visa", last4: "4242" } };
    const { root, send } = setup(ok({ state: "ready", logins: LOGINS, profile }));
    await renderMenu(root, { send, mode: "card" });
    expect(root.textContent).toContain("Payment card");
    expect(root.textContent).toContain("•••• 4242");
    expect(root.textContent).not.toContain("GitHub");
    root.querySelector<HTMLButtonElement>("button.row")!.click();
    await flush();
    expect(send).toHaveBeenCalledWith({ kind: "menu-fill-profile", profile: "card", itemId: "c" });
    await renderMenu(root, { send, mode: "identity" });
    expect(root.textContent).toContain("Fill name, address and contact");
  });

  it("offers every identity, the default first", async () => {
    const home = { id: "h", title: "Home", last4: null };
    const work = { id: "w", title: "Work", last4: null };
    const profile = { identity: work, card: null, identities: [work, home], cards: [] };
    const { root, send } = setup(ok({ state: "ready", logins: [], profile }));
    await renderMenu(root, { send, mode: "identity" });
    const rows = [...root.querySelectorAll<HTMLButtonElement>("button.row")];
    expect(rows.map((r) => r.textContent)).toEqual([expect.stringContaining("Work"), expect.stringContaining("Home")]);
    rows[1]!.click();
    await flush();
    expect(send).toHaveBeenCalledWith({ kind: "menu-fill-profile", profile: "identity", itemId: "h" });
    await renderMenu(root, { send, mode: "card" });
    expect(root.textContent).toContain("Choose a payment card");
  });

  it("says where to choose a card or identity when none is set", async () => {
    const { root, send } = setup(ok({ state: "ready", logins: [], profile: { identity: null, card: { id: "c", title: "Visa", last4: null } } }));
    await renderMenu(root, { send, mode: "identity" });
    expect(root.textContent).toContain("Choose an identity in Password365 › Profile");
    await renderMenu(root, { send, mode: "card" });
    expect(root.textContent).toContain("Fill card details");
    const old = setup(ok({ state: "ready", logins: [] }));
    await renderMenu(old.root, { send: old.send, mode: "card" });
    expect(old.root.textContent).toContain("Choose a payment card");
  });
});

describe("renderMenu — passkeys", () => {
  it("offers the passkeys a waiting sign-in can use above the logins", async () => {
    const passkeys = [{ item_id: "p1", title: "Example", user_name: "ana" }];
    const { root, send } = setup(ok({ state: "ready", logins: [], passkeys }));
    await renderMenu(root, { send, mode: "login" });
    expect(root.textContent).toContain("Passkeys");
    expect(root.textContent).toContain("Passkey · ana");
    expect(root.textContent).not.toContain("No saved logins");
    [...root.querySelectorAll<HTMLButtonElement>("button.row")][0]!.click();
    expect(send).toHaveBeenCalledWith({ kind: "menu-passkey-autofill", itemId: "p1" });
  });

  const click = async (root: HTMLElement, text: string) => {
    [...root.querySelectorAll<HTMLButtonElement>("button.row")].find((b) => b.textContent!.includes(text))!.click();
    await flush();
  };

  it("offers to save a passkey or to leave it to the browser", async () => {
    const { root, send } = setup(ok(null), { "menu-passkey-state": ok({ kind: "create", site: "example.com", user_name: "ana" }) });
    await renderMenu(root, { send, mode: "passkey" });
    expect(root.textContent).toContain("Save a passkey for example.com?");
    await click(root, "Save passkey in Password365");
    expect(send).toHaveBeenCalledWith({ kind: "menu-passkey-choose" });
    await click(root, "Use another device or method");
    expect(send).toHaveBeenCalledWith({ kind: "menu-passkey-other" });
  });

  it("lists the accounts to sign in with", async () => {
    const choices = [{ item_id: "i1", title: "Example", user_name: "ana" }];
    const { root, send } = setup(ok(null), { "menu-passkey-state": ok({ kind: "get", site: "example.com", choices }) });
    await renderMenu(root, { send, mode: "passkey" });
    expect(root.textContent).toContain("Sign in to example.com with a passkey");
    await click(root, "ana");
    expect(send).toHaveBeenCalledWith({ kind: "menu-passkey-choose", itemId: "i1" });
  });

  it("after a choice, asks to confirm it is you in the popup when the site requires it", async () => {
    vi.useFakeTimers();
    try {
      const choices = [{ item_id: "i1", title: "Example", user_name: "ana" }];
      const states = [ok({ kind: "get", site: "example.com", choices }), ok({ kind: "verify", site: "example.com" }), ok(null)];
      const root = document.createElement("main");
      const send = vi.fn(async (m: MenuMessage) =>
        m.kind === "menu-passkey-state" ? states.shift()! : m.kind === "menu-passkey-choose" ? ok("verify") : ok(true),
      );
      await renderMenu(root, { send, mode: "passkey", pollMs: 500 });
      [...root.querySelectorAll<HTMLButtonElement>("button.row")][0]!.click();
      await vi.advanceTimersByTimeAsync(0);
      expect(root.textContent).toContain("example.com asks to confirm it is you");
      [...root.querySelectorAll<HTMLButtonElement>("button.row")][0]!.click();
      expect(send).toHaveBeenCalledWith({ kind: "menu-passkey-unlock" });
      await vi.advanceTimersByTimeAsync(500);
      expect(root.textContent).not.toContain("confirm it is you");
    } finally {
      vi.useRealTimers();
    }
  });

  it("asks to unlock and carries on once the app is unlocked", async () => {
    vi.useFakeTimers();
    try {
      const states = [ok({ kind: "locked", site: "example.com" }), ok({ kind: "create", site: "example.com", user_name: "ana" })];
      const root = document.createElement("main");
      const send = vi.fn(async (m: MenuMessage) => (m.kind === "menu-passkey-state" ? states.shift()! : ok(true)));
      await renderMenu(root, { send, mode: "passkey", pollMs: 500 });
      expect(root.textContent).toContain("Unlock Password365 to use a passkey on example.com");
      [...root.querySelectorAll<HTMLButtonElement>("button.row")][0]!.click();
      expect(send).toHaveBeenCalledWith({ kind: "menu-passkey-unlock" });
      await vi.advanceTimersByTimeAsync(500);
      expect(root.textContent).toContain("Save a passkey for example.com?");
      expect(root.textContent).not.toContain("Unlock Password365");
    } finally {
      vi.useRealTimers();
    }
  });

  it("says when the request is over or failed", async () => {
    for (const [reply, text] of [
      [ok(null), "This passkey request has ended"],
      [fail("app_not_running"), "Open the Password365 app"],
    ] as const) {
      const { root, send } = setup(ok(null), { "menu-passkey-state": reply });
      await renderMenu(root, { send, mode: "passkey" });
      expect(root.textContent).toContain(text);
    }
  });
});
