import { useEffect, useMemo, useState } from "react";

import { translateOr, type Translate } from "../../../../i18n/translate";

import type { Client } from "../api/client";
import type { Category } from "../api/types";
import { useClient } from "../api/context";
import { useT } from "../i18n";

// The list never changes while the app runs: fetch it once per client.
const cache = new WeakMap<Client, Promise<Category[]>>();

function load(client: Client): Promise<Category[]> {
  let found = cache.get(client);
  if (!found) {
    found = client.categories().then((c) => c ?? []);
    cache.set(client, found);
  }
  return found;
}

/** Every category with its fields (empty until loaded), from the client in context. */
export function useCategories(): Category[] {
  return useCategoriesOf(useClient());
}

/** `categories` with their names in the interface language (the core's English name when untranslated). */
export function translateCategories(categories: Category[], t: Translate): Category[] {
  return categories.map((c) => ({
    ...c,
    label: translateOr(t, `categories.${c.kind}`, c.label),
    plural: translateOr(t, `categories.${c.kind}_plural`, c.plural),
  }));
}

/** [`useCategories`] for a given client. */
export function useCategoriesOf(client: Client): Category[] {
  const t = useT();
  const [categories, setCategories] = useState<Category[]>([]);
  useEffect(() => {
    let live = true;
    load(client).then(
      (c) => live && setCategories(c),
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [client]);
  return useMemo(() => translateCategories(categories, t), [categories, t]);
}

/** Display name of `kind` ("Credit Card"), or the id until the list is loaded. */
export function categoryLabel(categories: Category[], kind: string): string {
  return categories.find((c) => c.kind === kind)?.label ?? kind;
}
