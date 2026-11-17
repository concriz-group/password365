import type { PopupMessage, Reply } from "../shared/messages";
import { renderPopup } from "./view";

const ok = (result: unknown): Reply => ({ ok: true, result });
const fail = (code: string, message = code): Reply => ({ ok: false, error: { code, message } });

function setup(replies: Partial<Record<PopupMessage["kind"], Reply>>, tabId: number | null = 1) {
  document.body.innerHTML = `<main id="app"></main>`;
  const root = document.getElementById("app")!;
  const send = vi.fn(async (m: PopupMessage) => replies[m.kind] ?? ok(null));
  const deps = { send, tabId: tabId ?? undefined, copy: vi.fn(async () => {}), close: vi.fn() };
  return { root, deps, render: () => renderPopup(root, deps) };
}

const unlocked = ok({ initialized: true, unlocked: true });
const flush = () => new Promise((r) => setTimeout(r, 0));

describe("popup", () => {
  it("explains connection problems and lock states", async () => {
    for (const [reply, text] of [
      [fail("app_not_running"), "Open the Password365 app"],
      [fail("host_missing"), "npm run install-host"],
      [fail("weird", "something odd"), "something odd"],
      [ok({ initialized: false, unlocked: false }), "Create your account"],
      [ok({ initialized: true, unlocked: false }), "Password365 is locked"],
    ] as const) {
      const { root, render } = setup({ status: reply });
      await render();
      expect(root.textContent).toContain(text);
    }
  });

  it("closes after unlocking when a page's passkey prompt is waiting", async () => {
    const { root, deps, render } = setup({ status: ok({ initialized: true, unlocked: false }), unlock: ok(true), "passkey-waiting": ok(true) });
    await render();
    root.querySelector<HTMLInputElement>("input[type=password]")!.value = "pw";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    await flush();
    expect(deps.send).toHaveBeenCalledWith({ kind: "passkey-waiting", tabId: 1 });
    expect(deps.close).toHaveBeenCalled();
  });

  it("confirms it is you for a passkey that needs it, then closes", async () => {
    const { root, deps, render } = setup({ status: unlocked, "passkey-verifying": ok(true) });
    deps.send.mockImplementation(async (m: PopupMessage) => {
      if (m.kind === "status") return unlocked;
      if (m.kind === "passkey-verifying") return ok(true);
      if (m.kind === "passkey-verify") return m.password === "master" ? ok(true) : fail("wrong_credentials");
      return ok(null);
    });
    await render();
    expect(root.textContent).toContain("Confirm it is you to use the passkey");
    const input = root.querySelector<HTMLInputElement>("input[type=password]")!;
    const form = root.querySelector("form")!;
    input.value = "nope";
    form.dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    expect(root.querySelector(".error")!.textContent).not.toBe("");
    expect(deps.close).not.toHaveBeenCalled();
    input.value = "master";
    form.dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    expect(deps.send).toHaveBeenCalledWith({ kind: "passkey-verify", tabId: 1, password: "master" });
    expect(deps.close).toHaveBeenCalled();
  });

  it("unlocks with the master password, like the 1Password popup", async () => {
    let unlockedNow = false;
    const { root, deps, render } = setup({});
    deps.send.mockImplementation(async (m: PopupMessage) => {
      if (m.kind === "status") return ok({ initialized: true, unlocked: unlockedNow });
      if (m.kind === "unlock") {
        unlockedNow = m.password === "right password";
        return unlockedNow ? ok(true) : fail("validation", "wrong master password");
      }
      return ok([]);
    });
    await render();
    const input = root.querySelector<HTMLInputElement>("input[type=password]")!;
    expect(input.getAttribute("aria-label")).toBe("Master password");
    const form = root.querySelector("form")!;

    form.dispatchEvent(new Event("submit", { cancelable: true })); // empty: ignored
    expect(deps.send).not.toHaveBeenCalledWith(expect.objectContaining({ kind: "unlock" }));

    input.value = "nope";
    form.dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    expect(root.textContent).toContain("wrong master password");
    expect(input.value).toBe("");

    input.value = "right password";
    form.dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    await flush();
    expect(deps.send).toHaveBeenCalledWith({ kind: "unlock", password: "right password" });
    expect(root.textContent).toContain("Logins for this site");
  });

  it("lists logins and fills one", async () => {
    const logins = [{ id: "i1", title: "<b>GitHub</b>", username: "octo", match_level: "origin" }];
    const { root, deps, render } = setup({ status: unlocked, logins: ok(logins), fill: ok(true) });
    await render();
    expect(root.querySelector("strong")!.textContent).toBe("<b>GitHub</b>"); // never parsed as HTML
    root.querySelector<HTMLButtonElement>("li button")!.click();
    await flush();
    expect(deps.send).toHaveBeenCalledWith({ kind: "fill", tabId: 1, itemId: "i1" });
    expect(deps.close).toHaveBeenCalled();
  });

  it("shows fill errors on the button", async () => {
    const logins = [{ id: "i1", title: "A", username: null, match_level: "site" }];
    const { root, render } = setup({ status: unlocked, logins: ok(logins), fill: fail("validation", "not for this site") });
    await render();
    const button = root.querySelector<HTMLButtonElement>("li button")!;
    button.click();
    await flush();
    expect(button.textContent).toBe("not for this site");
  });

  it("handles empty lists, missing tabs and list errors", async () => {
    let s = setup({ status: unlocked, logins: ok([]) });
    await s.render();
    expect(s.root.textContent).toContain("No saved logins for this site.");
    s = setup({ status: unlocked }, null);
    await s.render();
    expect(s.root.textContent).toContain("No page to fill.");
    s = setup({ status: unlocked, logins: ok(null) });
    await s.render();
    expect(s.root.textContent).toContain("No saved logins for this site.");
    s = setup({ status: unlocked, logins: fail("no_page", "This page cannot be filled") });
    await s.render();
    expect(s.root.textContent).toContain("This page cannot be filled");
  });

  it("generates and copies a password", async () => {
    const { root, deps, render } = setup({ status: unlocked, logins: ok([]), generate: ok({ value: "S3cret!" }) });
    await render();
    const [generate, copy] = root.querySelectorAll<HTMLButtonElement>(".generator button");
    expect(copy!.hidden).toBe(true);
    generate!.click();
    await flush();
    expect(root.querySelector("code")!.textContent).toBe("S3cret!");
    copy!.click();
    await flush();
    expect(deps.copy).toHaveBeenCalledWith("S3cret!");
    expect(copy!.textContent).toBe("Copied");
  });

  it("shows generator errors", async () => {
    const { root, render } = setup({ status: unlocked, logins: ok([]), generate: fail("locked", "the vault is locked") });
    await render();
    root.querySelector<HTMLButtonElement>(".generator button")!.click();
    await flush();
    expect(root.querySelector("code")!.textContent).toBe("Password365 is locked.");
  });
});
