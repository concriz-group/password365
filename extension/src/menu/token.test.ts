import { receiveToken } from "./token";

/** A window-like target whose parent is `parent`. */
function menuWindow(parent: object) {
  const target = new EventTarget() as EventTarget & { parent: object };
  target.parent = parent;
  return target as unknown as Window;
}
const message = (source: object | null, data: unknown) => {
  const e = new MessageEvent("message", { data });
  Object.defineProperty(e, "source", { value: source });
  return e;
};

describe("receiveToken", () => {
  it("takes the token the parent window sends, once", async () => {
    const parent = {};
    const win = menuWindow(parent);
    const token = receiveToken(win);
    win.dispatchEvent(message(parent, { password365Token: "first" }));
    win.dispatchEvent(message(parent, { password365Token: "second" }));
    await expect(token).resolves.toBe("first");
  });

  it("ignores other windows and anything that is not a token", async () => {
    const parent = {};
    const win = menuWindow(parent);
    const token = receiveToken(win);
    win.dispatchEvent(message({}, { password365Token: "from another frame" }));
    win.dispatchEvent(message(parent, null));
    win.dispatchEvent(message(parent, { password365Token: 7 }));
    win.dispatchEvent(message(parent, { password365Token: "" }));
    win.dispatchEvent(message(parent, { password365Filter: "ali" }));
    win.dispatchEvent(message(parent, { password365Token: "real" }));
    await expect(token).resolves.toBe("real");
  });
});
