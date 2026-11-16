import { displayShortcut, fromKeyEvent } from "./shortcut";

const press = (key: string, code: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({
  key,
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("shortcuts", () => {
  it("records key presses with modifiers", () => {
    expect(fromKeyEvent(press("L", "KeyL", { shiftKey: true, metaKey: true }))).toBe("Shift+Super+KeyL");
    expect(fromKeyEvent(press(" ", "Space", { ctrlKey: true, altKey: true }))).toBe("Ctrl+Alt+Space");
    expect(fromKeyEvent(press("F9", "F9"))).toBe("F9");
    expect(fromKeyEvent(press("a", "KeyA"))).toBeNull();
    expect(fromKeyEvent(press("Shift", "ShiftLeft", { shiftKey: true }))).toBeNull();
  });

  it("shows them the way each system writes them", () => {
    expect(displayShortcut("Shift+Super+KeyL", true)).toBe("⇧⌘L");
    expect(displayShortcut("Ctrl+Alt+Digit1", true)).toBe("⌃⌥1");
    expect(displayShortcut("Shift+Super+Space", false)).toBe("Shift+Win+Space");
    expect(displayShortcut("", false)).toBe("");
  });
});
