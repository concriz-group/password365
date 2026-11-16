import { useCallback, useEffect, useState } from "react";

import { useClient } from "../api/context";
import type { DeletedItem } from "../api/types";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  /** Called after an item is restored, so lists and counts refresh. */
  onRestored: () => void;
}

const DAY_MS = 24 * 60 * 60 * 1000;

/** Days left before `purgeAt`, never below 0. */
export function daysLeft(purgeAt: number, now: number): number {
  return Math.max(0, Math.ceil((purgeAt - now) / DAY_MS));
}

/** Recently Deleted: restore an item, or delete it for good. Items go away on their own after 30 days. */
export function RecentlyDeleted({ onRestored }: Props) {
  const client = useClient();
  const t = useT();
  const [items, setItems] = useState<DeletedItem[] | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  const load = useCallback(async () => setItems(await client.deletedItems()), [client]);
  const loading = useAction(load);
  const restore = useAction(async (id: string) => {
    await client.restoreItem(id);
    await load();
    onRestored();
  });
  const purge = useAction(async (id: string) => {
    await client.deletePermanently(id);
    setConfirming(null);
    await load();
  });

  const { run: reload } = loading;
  useEffect(() => {
    void reload();
  }, [reload]);

  const now = Date.now();
  return (
    <article className="recently-deleted">
      <h1>{t("recentlyDeleted.title")}</h1>
      <p className="hint">{t("recentlyDeleted.hint")}</p>
      <ErrorMessage error={loading.error ?? restore.error ?? purge.error} />
      {items?.length === 0 && <p className="muted">{t("recentlyDeleted.empty")}</p>}
      <ul className="deleted-list">
        {items?.map((item) => (
          <li key={item.id}>
            <span className="deleted-title">
              <strong>{item.title}</strong>
              <span className="muted">
                {item.username ?? ""} · {t("recentlyDeleted.daysLeft", { count: daysLeft(item.purge_at, now) })}
              </span>
            </span>
            <span className="deleted-actions">
              <button type="button" disabled={restore.busy} onClick={() => void restore.run(item.id)}>
                {t("recentlyDeleted.restore")}
              </button>
              {confirming === item.id ? (
                <>
                  <button type="button" className="danger" onClick={() => void purge.run(item.id)}>
                    {t("recentlyDeleted.deletePermanently")}
                  </button>
                  <button type="button" className="ghost" onClick={() => setConfirming(null)}>
                    {t("common.cancel")}
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  className="ghost"
                  aria-label={t("recentlyDeleted.deleteAria", { title: item.title })}
                  onClick={() => setConfirming(item.id)}
                >
                  {t("recentlyDeleted.delete")}
                </button>
              )}
            </span>
          </li>
        ))}
      </ul>
    </article>
  );
}
