import { useState } from "react";

import type { Category } from "../api/types";
import { useT } from "../i18n";
import { ItemIcon } from "./ItemIcon";

/** The six big tiles at the top, like 1Password's (Document is not supported yet). */
const FEATURED = ["login", "note", "card", "identity", "password", "api_credential"];

interface Props {
  categories: Category[];
  onPick: (kind: string) => void;
  onCancel: () => void;
}

/** "What would you like to add?": search, the main categories as tiles, the rest as a list. */
export function NewItemPicker({ categories, onPick, onCancel }: Props) {
  const t = useT();
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  const matching = categories.filter((c) => c.label.toLowerCase().includes(q));
  const featured = FEATURED.flatMap((k) => matching.filter((c) => c.kind === k));
  const others = matching
    .filter((c) => !FEATURED.includes(c.kind))
    .sort((a, b) => a.label.localeCompare(b.label));

  const tile = (c: Category, className: string) => (
    <button key={c.kind} type="button" className={className} onClick={() => onPick(c.kind)}>
      <ItemIcon title={c.label} kind={c.kind} />
      {c.label}
    </button>
  );

  return (
    <section className="new-item-picker" aria-label={t("newItem.label")}>
      <header className="editor-header">
        <h1>{t("newItem.title")}</h1>
        <div className="editor-actions">
          <button type="button" className="ghost" onClick={onCancel}>
            {t("common.cancel")}
          </button>
        </div>
      </header>
      <input
        type="search"
        aria-label={t("newItem.search")}
        placeholder={t("newItem.search")}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        autoFocus
      />
      {featured.length > 0 && <div className="category-tiles">{featured.map((c) => tile(c, "category-tile"))}</div>}
      {others.length > 0 && <div className="category-list">{others.map((c) => tile(c, "category-row"))}</div>}
      {matching.length === 0 && <p className="muted">{t("newItem.noMatch", { query })}</p>}
    </section>
  );
}
