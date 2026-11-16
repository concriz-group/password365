import { COLORS, LIBRARY, SVGS, builtinIcon, categoryIcon, parseIcon } from "./icons";

describe("icons", () => {
  it("loads the SVG files and every category's icon", () => {
    expect(Object.keys(SVGS).length).toBeGreaterThan(30);
    expect(SVGS.home).toMatch(/^<svg[\s\S]*<\/svg>$/);
    expect(LIBRARY).toContain("home");
    expect(COLORS.length).toBeGreaterThan(3);
    expect(categoryIcon("card")).toEqual({ name: "card", color: "#0891b2" });
    expect(categoryIcon("nope")).toBeNull();
  });

  it("reads stored icons and refuses unknown ones", () => {
    expect(parseIcon("data:image/png;base64,AA==")).toEqual({ type: "picture", src: "data:image/png;base64,AA==" });
    expect(parseIcon(builtinIcon("home", "#16a34a"))).toEqual({ type: "builtin", name: "home", color: "#16a34a" });
    expect(parseIcon("builtin:home")).toEqual({ type: "builtin", name: "home", color: COLORS[0] });
    expect(parseIcon("builtin:not_an_icon:#000000")).toBeNull();
    expect(parseIcon("javascript:alert(1)")).toBeNull();
    expect(parseIcon(null)).toBeNull();
  });
});
