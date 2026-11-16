import type { Category, ItemKind, ItemSummary } from "../api/types";
import { ItemIcon } from "./ItemIcon";
import { SORT_LABELS, type Sort } from "../lib/scope";
import { useT } from "../i18n";

interface Props {
  items: ItemSummary[];
  /** Items' own icons by id. */
  icons?: Record<string, string>;
  selectedId: string | null;
  onSelect: (id: string) => void;
  query: string;
  onQuery: (query: string) => void;
  /** Categories in the current list (the "All Categories" menu). */
  categories?: Category[];
  category?: ItemKind | null;
  onCategory?: (category: ItemKind | null) => void;
  sort?: Sort;
  onSort?: (sort: Sort) => void;
}

/** Search box, category filter, sort order and the list of items. */
export function ItemList({
  items,
  icons = {},
  selectedId,
  onSelect,
  query,
  onQuery,
  categories = [],
  category = null,
  onCategory,
  sort = "title",
  onSort,
}: Props) {
  const t = useT();
  return (
    <section className="item-list" aria-label={t("list.items")}>
      <input
        type="search"
        aria-label={t("common.search")}
        placeholder={t("common.search")}
        value={query}
        onChange={(e) => onQuery(e.target.value)}
      />
      <div className="list-filters">
        {onCategory && (
          <select
            aria-label={t("list.category")}
            value={category ?? ""}
            onChange={(e) => onCategory((e.target.value || null) as ItemKind | null)}
          >
            <option value="">{t("list.allCategories")}</option>
            {categories.map((c) => (
              <option key={c.kind} value={c.kind}>
                {c.plural}
              </option>
            ))}
            {category && !categories.some((c) => c.kind === category) && <option value={category}>{category}</option>}
          </select>
        )}
        {onSort && (
          <select aria-label={t("list.sortBy")} value={sort} onChange={(e) => onSort(e.target.value as Sort)}>
            {(Object.keys(SORT_LABELS) as Sort[]).map((s) => (
              <option key={s} value={s}>
                {t(SORT_LABELS[s])}
              </option>
            ))}
          </select>
        )}
      </div>
      {items.length === 0 ? (
        <p className="muted empty">{query ? t("list.noMatches") : t("list.empty")}</p>
      ) : (
        <ul>
          {items.map((item) => (
            <li key={item.id}>
              <button
                type="button"
                className={item.id === selectedId ? "row active" : "row"}
                onClick={() => onSelect(item.id)}
              >
                <ItemIcon title={item.title} icon={icons[item.id]} kind={item.kind} />
                <span className="row-text">
                  <span className="row-title">
                    {item.title}
                    {item.needs_review && <span className="badge">{t("list.review")}</span>}
                  </span>
                  <span className="muted">{item.username ?? item.urls[0] ?? categories.find((c) => c.kind === item.kind)?.label ?? item.kind}</span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
