import { useCallback, useEffect, useMemo, useState } from "react";

import type { Client } from "../api/client";
import type { ItemKind, ItemSummary, Tag, Vault } from "../api/types";
import { useCategoriesOf } from "./useCategories";
import { ALL, type Scope, type Sort, presentCategories, scopeVault, visibleItems } from "../lib/scope";

/** Loads vaults, tags and the item list; reloads when the search or vault changes. */
export function useVaultData(client: Client) {
  const [vaults, setVaults] = useState<Vault[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [loaded, setLoaded] = useState<ItemSummary[]>([]);
  const [scope, setScope] = useState<Scope>(ALL);
  const [category, setCategory] = useState<ItemKind | null>(null);
  const [sort, setSort] = useState<Sort>("title");
  const [query, setQuery] = useState("");
  const [error, setError] = useState<unknown>(null);
  const vaultId = scopeVault(scope);
  const allCategories = useCategoriesOf(client);

  const reload = useCallback(async () => {
    try {
      const list =
        scope.type === "archive" ? client.archivedItems(query) : client.items(query, vaultId ?? undefined);
      const [v, i, t] = await Promise.all([client.vaults(), list, client.tags()]);
      setVaults(v);
      setLoaded(i);
      setTags(t);
      setError(null);
    } catch (e) {
      setError(e);
    }
  }, [client, query, vaultId, scope.type]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const items = useMemo(() => visibleItems(loaded, scope, category, sort), [loaded, scope, category, sort]);
  const categories = useMemo(
    () => presentCategories(visibleItems(loaded, scope, null, sort), allCategories),
    [loaded, scope, sort, allCategories],
  );
  const setVaultId = useCallback((id: string | null) => setScope(id === null ? ALL : { type: "vault", id }), []);

  return {
    vaults,
    tags,
    items,
    categories,
    category,
    setCategory,
    sort,
    setSort,
    scope,
    setScope,
    vaultId,
    setVaultId,
    query,
    setQuery,
    error,
    reload,
  };
}
