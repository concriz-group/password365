import { useState } from "react";

import { useT } from "../i18n";
import { IMPORT_SOURCES, SOURCE_GROUPS, type ImportSource, matchingSources } from "../lib/importSources";

/** A source's shown name: the product's, or the catalog's for generic ones. */
export function useSourceName(): (source: ImportSource) => string {
  const t = useT();
  return (source) => source.name ?? t(`migrate.name_${source.id}`);
}

/** "Migrate your data", like 1Password: pick where the data comes from. */
export function MigratePicker({ onPick }: { onPick: (source: ImportSource) => void }) {
  const t = useT();
  const nameOf = useSourceName();
  const [query, setQuery] = useState("");
  const shown = matchingSources(IMPORT_SOURCES, query, nameOf);
  return (
    <div className="migrate">
      <label htmlFor="migrate-search">{t("migrate.question")}</label>
      <input
        id="migrate-search"
        type="search"
        value={query}
        placeholder={t("migrate.search")}
        onChange={(e) => setQuery(e.target.value)}
      />
      {shown.length === 0 && <p className="hint">{t("migrate.noMatch")}</p>}
      {SOURCE_GROUPS.map((group) => {
        const sources = shown.filter((s) => s.group === group);
        if (sources.length === 0) return null;
        return (
          <section key={group} aria-label={t(`migrate.group_${group}`)}>
            <h3>{t(`migrate.group_${group}`)}</h3>
            <div className="tiles">
              {sources.map((source) => (
                <button type="button" className={source.name ? "tile" : "tile generic"} key={source.id} onClick={() => onPick(source)}>
                  {source.name && (
                    <span className="avatar" aria-hidden="true">
                      {source.name.replace(/^\d/, "").charAt(0) || source.name.charAt(0)}
                    </span>
                  )}
                  <span>{nameOf(source)}</span>
                </button>
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}
