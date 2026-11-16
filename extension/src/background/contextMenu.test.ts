import { CONTEXT_MENUS, handleContextMenuClick, SUGGEST_MENU_ID } from "./contextMenu";

function deps() {
  return {
    generate: vi.fn(async () => ({ value: "Gen3rated!", bits: 120, strength: "very_strong" })),
    send: vi.fn(async () => {}),
    openPopup: vi.fn(async () => {}),
  };
}

describe("context menu", () => {
  it("defines Password365 > Suggest password on editable fields of web pages", () => {
    const [parent, suggest] = CONTEXT_MENUS;
    expect(parent).toMatchObject({ title: "Password365", contexts: ["editable"] });
    expect(suggest).toMatchObject({ id: SUGGEST_MENU_ID, parentId: parent!.id, title: "Suggest password" });
  });

  it("generates a password and puts it into the field that was right-clicked", async () => {
    const d = deps();
    await handleContextMenuClick({ menuItemId: SUGGEST_MENU_ID, frameId: 0 }, 7, d);
    expect(d.send).toHaveBeenCalledWith(7, { kind: "suggest-at-target", password: "Gen3rated!" });
  });

  it("ignores other items, sub-frames and clicks without a tab", async () => {
    const d = deps();
    await handleContextMenuClick({ menuItemId: "other", frameId: 0 }, 7, d);
    await handleContextMenuClick({ menuItemId: SUGGEST_MENU_ID, frameId: 3 }, 7, d);
    await handleContextMenuClick({ menuItemId: SUGGEST_MENU_ID, frameId: 0 }, undefined, d);
    expect(d.generate).not.toHaveBeenCalled();
  });

  it("opens the popup (which explains the problem) when the app can't answer", async () => {
    const d = deps();
    d.generate.mockRejectedValueOnce(new Error("app_not_running"));
    await handleContextMenuClick({ menuItemId: SUGGEST_MENU_ID, frameId: 0 }, 7, d);
    expect(d.openPopup).toHaveBeenCalled();
    d.generate.mockRejectedValueOnce(new Error("app_not_running"));
    d.openPopup.mockRejectedValueOnce(new Error("no window"));
    await expect(handleContextMenuClick({ menuItemId: SUGGEST_MENU_ID, frameId: 0 }, 7, d)).resolves.toBeUndefined();
  });
});
