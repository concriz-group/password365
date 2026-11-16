import { useState, type FormEvent } from "react";

import type { Category, Tag, Vault } from "../api/types";
import { ALL, type Scope, sameScope } from "../lib/scope";
import { useT } from "../i18n";
import { ItemIcon } from "./ItemIcon";

interface Props {
  vaults: Vault[];
  tags?: Tag[];
  /** Categories to list (Settings › Appearance › Always show in sidebar). */
  categories?: Category[];
  /** The category the list is filtered by. */
  category?: string | null;
  onCategory?: (kind: string | null) => void;
  selected: Scope;
  onSelect: (scope: Scope) => void;
  onCreate: (name: string) => Promise<void>;
  /** Recently Deleted is open (no vault is highlighted then). */
  showingDeleted?: boolean;
  onShowDeleted?: () => void;
  /** Watchtower is open (no vault is highlighted then). */
  showingWatchtower?: boolean;
  onShowWatchtower?: () => void;
  /** Profile is open (no vault is highlighted then). */
  showingProfile?: boolean;
  onShowProfile?: () => void;
}

/** Navigation like 1Password's: Profile, All Items, Favorites, Watchtower, vaults (+ new), tags, Archive, Recently Deleted. */
export function Sidebar({ vaults, tags = [], categories = [], category = null, onCategory, selected, onSelect, onCreate, showingDeleted = false, onShowDeleted, showingWatchtower = false, onShowWatchtower, showingProfile = false, onShowProfile }: Props) {
  const t = useT();
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const total = vaults.reduce((sum, v) => sum + v.item_count, 0);

  const special = showingDeleted || showingWatchtower || showingProfile;
  const navClass = (scope: Scope) => (!special && sameScope(selected, scope) ? "nav active" : "nav");

  async function submit(e: FormEvent) {
    e.preventDefault();
    await onCreate(name);
    setName("");
    setAdding(false);
  }

  return (
    <nav className="sidebar" aria-label={t("sidebar.vaults")}>
      {onShowProfile && (
        <button type="button" className={showingProfile ? "nav active" : "nav"} onClick={onShowProfile}>
          {t("sidebar.profile")}
        </button>
      )}
      <button type="button" className={navClass(ALL)} onClick={() => onSelect(ALL)}>
        {t("sidebar.allItems")} <span className="count">{total}</span>
      </button>
      <button type="button" className={navClass({ type: "favorites" })} onClick={() => onSelect({ type: "favorites" })}>
        {t("sidebar.favorites")}
      </button>
      {onShowWatchtower && (
        <button type="button" className={showingWatchtower ? "nav active" : "nav"} onClick={onShowWatchtower}>
          {t("sidebar.watchtower")}
        </button>
      )}
      {onCategory && categories.length > 0 && (
        <>
          <h2>{t("sidebar.categories")}</h2>
          {categories.map((c) => (
            <button
              key={c.kind}
              type="button"
              className={!special && category === c.kind ? "nav active" : "nav"}
              aria-pressed={category === c.kind}
              onClick={() => onCategory(category === c.kind ? null : c.kind)}
            >
              <ItemIcon title={c.plural} kind={c.kind} />
              {c.plural}
            </button>
          ))}
        </>
      )}
      <h2>{t("sidebar.vaults")}</h2>
      {vaults.map((v) => (
        <button
          key={v.id}
          type="button"
          className={navClass({ type: "vault", id: v.id })}
          onClick={() => onSelect({ type: "vault", id: v.id })}
        >
          {v.name} <span className="count">{v.item_count}</span>
        </button>
      ))}
      {adding ? (
        <form onSubmit={submit} className="add-vault">
          <input aria-label={t("sidebar.vaultName")} value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          <button type="submit" disabled={!name.trim()}>
            {t("sidebar.add")}
          </button>
          <button type="button" className="ghost" onClick={() => setAdding(false)}>
            {t("common.cancel")}
          </button>
        </form>
      ) : (
        <button type="button" className="link" onClick={() => setAdding(true)}>
          {t("sidebar.newVault")}
        </button>
      )}
      {tags.length > 0 && (
        <>
          <h2>{t("sidebar.tags")}</h2>
          {tags.map((tag) => (
            <button
              key={tag.name}
              type="button"
              className={navClass({ type: "tag", tag: tag.name })}
              onClick={() => onSelect({ type: "tag", tag: tag.name })}
            >
              {tag.name} <span className="count">{tag.count}</span>
            </button>
          ))}
        </>
      )}
      <button
        type="button"
        className={navClass({ type: "archive" }) + " archive-nav"}
        onClick={() => onSelect({ type: "archive" })}
      >
        {t("sidebar.archive")}
      </button>
      {onShowDeleted && (
        <button
          type="button"
          className={showingDeleted ? "nav deleted-nav active" : "nav deleted-nav"}
          onClick={onShowDeleted}
        >
          {t("sidebar.recentlyDeleted")}
        </button>
      )}
    </nav>
  );
}
