import { FILL_COMMAND, handleShortcut } from "./shortcut";

function setup(status: unknown, logins: unknown[] = []) {
  const request = vi.fn(async (body: { type: string }) => {
    if (body.type === "status") return status;
    if (body.type === "logins_for_url") return logins;
    if (body.type === "fill") return { username: "u", password: "p" };
    throw new Error(body.type);
  });
  const tabs = { url: vi.fn(async () => "https://github.com/login"), send: vi.fn(async () => {}) };
  const openPopup = vi.fn(async () => {});
  return { deps: { bridge: { request } as never, tabs, openPopup }, request, tabs, openPopup };
}
const unlocked = { initialized: true, unlocked: true };
const login = (id: string) => ({ id, title: id, username: null, match_level: "site" });

describe("fill shortcut", () => {
  it("fills right away when the site has exactly one login", async () => {
    const { deps, tabs } = setup(unlocked, [login("only")]);
    await handleShortcut(FILL_COMMAND, 7, deps);
    expect(tabs.send).toHaveBeenCalledWith(7, { kind: "fill", username: "u", password: "p" });
  });

  it("opens the inline menu (keyboard-focused) when there are several or none", async () => {
    for (const logins of [[login("a"), login("b")], []]) {
      const { deps, tabs } = setup(unlocked, logins);
      await handleShortcut(FILL_COMMAND, 7, deps);
      expect(tabs.send).toHaveBeenCalledWith(7, { kind: "open-menu" });
    }
  });

  it("opens the popup to unlock, or to explain a problem", async () => {
    const locked = setup({ initialized: true, unlocked: false });
    await handleShortcut(FILL_COMMAND, 7, locked.deps);
    expect(locked.openPopup).toHaveBeenCalled();

    const broken = setup(unlocked);
    broken.request.mockRejectedValueOnce(new Error("app_not_running"));
    broken.openPopup.mockRejectedValueOnce(new Error("no window"));
    await expect(handleShortcut(FILL_COMMAND, 7, broken.deps)).resolves.toBeUndefined();
    expect(broken.openPopup).toHaveBeenCalled();
  });

  it("ignores other commands and missing tabs", async () => {
    const { deps, request } = setup(unlocked);
    await handleShortcut("other", 7, deps);
    await handleShortcut(FILL_COMMAND, undefined, deps);
    expect(request).not.toHaveBeenCalled();
  });
});
