import { useState } from "react";

import type { Item, Vault } from "../api/types";
import { useT } from "../i18n";

interface Props {
  item: Item;
  /** Vaults the item can move to (its own is left out). */
  vaults: Vault[];
  onFavorite: (favorite: boolean) => void;
  onMove: (vaultId: string) => void;
  onDuplicate: () => void;
  onArchive: () => void;
  onDelete: () => void;
}

/** The item's "⋮" menu, like 1Password's: favorites, Move…, Duplicate, Archive, Delete. */
export function ItemMenu({ item, vaults, onFavorite, onMove, onDuplicate, onArchive, onDelete }: Props) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [moving, setMoving] = useState(false);
  const targets = vaults.filter((v) => v.id !== item.vault_id);
  const run = (action: () => void) => () => {
    setOpen(false);
    setMoving(false);
    action();
  };
  return (
    <div className="item-menu" onKeyDown={(e) => e.key === "Escape" && setOpen(false)}>
      <button
        type="button"
        className="ghost"
        aria-label={t("itemMenu.more")}
        aria-expanded={open}
        onClick={() => {
          setOpen((o) => !o);
          setMoving(false);
        }}
      >
        ⋮
      </button>
      {open && (
        <div className="add-field-menu item-menu-list" role="menu">
          {moving ? (
            <>
              <p className="menu-heading">{t("itemMenu.moveTo")}</p>
              {targets.map((v) => (
                <button type="button" role="menuitem" key={v.id} onClick={run(() => onMove(v.id))}>
                  {v.name}
                </button>
              ))}
            </>
          ) : (
            <>
              <button type="button" role="menuitem" onClick={run(() => onFavorite(!item.favorite))}>
                {t(item.favorite ? "itemDetail.removeFavorite" : "itemDetail.addFavorite")}
              </button>
              {targets.length > 0 && (
                <button type="button" role="menuitem" onClick={() => setMoving(true)}>
                  {t("itemMenu.move")}
                </button>
              )}
              <button type="button" role="menuitem" onClick={run(onDuplicate)}>
                {t("itemMenu.duplicate")}
              </button>
              <hr />
              {!item.archived && (
                <button type="button" role="menuitem" onClick={run(onArchive)}>
                  {t("itemDetail.archive")}
                </button>
              )}
              <button type="button" role="menuitem" className="danger-text" onClick={run(onDelete)}>
                {t("common.delete")}
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
