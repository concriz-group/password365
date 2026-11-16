import { categories, summary } from "../test/fixtures";
import { ALL, presentCategories, sameScope, scopeVault, visibleItems } from "./scope";

const a = { ...summary, id: "a", title: "alpha", updated_at: 1 };
const b = { ...summary, id: "b", title: "Beta", updated_at: 3 };
const c = { ...summary, id: "c", title: "Gamma", updated_at: 3, kind: "note" as const };

describe("scope", () => {
  it("only sends vault scopes to the backend", () => {
    expect(scopeVault({ type: "vault", id: "v" })).toBe("v");
    expect(scopeVault(ALL)).toBeNull();
    expect(sameScope({ type: "tag", tag: "x" }, { type: "tag", tag: "x" })).toBe(true);
    expect(sameScope(ALL, { type: "favorites" })).toBe(false);
  });

  it("sorts by title ignoring case, or newest first with title as tie-break", () => {
    expect(visibleItems([b, c, a], ALL, null, "title").map((i) => i.id)).toEqual(["a", "b", "c"]);
    expect(visibleItems([a, c, b], ALL, null, "updated").map((i) => i.id)).toEqual(["b", "c", "a"]);
  });

  it("lists the categories present, in menu order", () => {
    expect(presentCategories([c, a], categories).map((k) => k.kind)).toEqual(["login", "note"]);
    expect(presentCategories([], categories)).toEqual([]);
  });
});
