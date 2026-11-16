// What the item list shows: the sidebar choice, the category filter and the sort order.

import type { Category, ItemKind, ItemSummary } from "../api/types";

/** Sidebar selection. */
export type Scope =
  | { type: "all" }
  | { type: "favorites" }
  | { type: "vault"; id: string }
  | { type: "tag"; tag: string }
  | { type: "archive" };

export const ALL: Scope = { type: "all" };

/** The vault the backend should filter by, if any. */
export function scopeVault(scope: Scope): string | null {
  return scope.type === "vault" ? scope.id : null;
}

export function sameScope(a: Scope, b: Scope): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export type Sort = "title" | "updated";

/** Translation key of each sort order's name. */
export const SORT_LABELS: Record<Sort, string> = {
  title: "list.sortTitle",
  updated: "list.sortUpdated",
};

/** Applies the sidebar scope (vaults are filtered by the backend), the category and the sort. */
export function visibleItems(items: ItemSummary[], scope: Scope, category: ItemKind | null, sort: Sort): ItemSummary[] {
  const shown = items.filter(
    (i) =>
      (scope.type !== "favorites" || i.favorite) &&
      (scope.type !== "tag" || i.tags.includes(scope.tag)) &&
      (category === null || i.kind === category),
  );
  const byTitle = (a: ItemSummary, b: ItemSummary) => a.title.localeCompare(b.title, undefined, { sensitivity: "base" });
  return shown.sort(sort === "title" ? byTitle : (a, b) => b.updated_at - a.updated_at || byTitle(a, b));
}

/** Categories that occur in `items`, in the order of `all`. */
export function presentCategories(items: ItemSummary[], all: Category[]): Category[] {
  const kinds = new Set(items.map((i) => i.kind));
  return all.filter((c) => kinds.has(c.kind));
}
