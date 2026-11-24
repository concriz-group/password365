import { useCallback, useEffect, useState } from "react";

import { useClient } from "../api/context";
import type { DuplicateGroup } from "../api/types";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  onOpenItem: (id: string) => void;
  /** Items were merged: the list must reload. */
  onMerged: () => void;
  /** Changes when items may have changed (sync). */
  refreshKey?: number;
}

/**
 * Review duplicates: logins that look like one account stored more than once (same username, same
 * site or the same brand's other country sites). The person picks the one to keep; the others'
 * websites are added to it and they go to Recently Deleted. Nothing happens without a click.
 */
export function DuplicatesPanel({ onOpenItem, onMerged, refreshKey = 0 }: Props) {
  const client = useClient();
  const t = useT();
  const [groups, setGroups] = useState<DuplicateGroup[] | null>(null);
  const [error, setError] = useState<unknown>(null);
  // Per group (by its first member id): the login chosen to keep.
  const [keep, setKeep] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const load = useCallback(() => client.duplicateGroups().then(setGroups, setError), [client]);
  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  async function merge(group: DuplicateGroup) {
    const id = group.members[0]!.id;
    const kept = keep[id] ?? id;
    setBusy(id);
    setError(null);
    try {
      await client.mergeDuplicates(
        kept,
        group.members.filter((m) => m.id !== kept).map((m) => m.id),
      );
      onMerged();
      await load();
    } catch (reason) {
      setError(reason);
    } finally {
      setBusy(null);
    }
  }

  // The search matches the site, the username and each login's title or websites (any case).
  const needle = query.trim().toLowerCase();
  const shown = !groups || !needle
    ? groups
    : groups.filter((g) =>
        [g.site, g.username, ...g.members.flatMap((m) => [m.title, ...m.urls])].some((text) => text.toLowerCase().includes(needle)),
      );

  if (!groups || !shown) return error ? <ErrorMessage error={error} /> : <p className="muted">{t("duplicates.checking")}</p>;

  return (
    <section className="duplicates" aria-label={t("duplicates.title")}>
      <h1>{t("duplicates.title")}</h1>
      <p className="muted">{t("duplicates.intro")}</p>
      {error ? <ErrorMessage error={error} /> : null}
      {groups.length > 0 && (
        <input
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          aria-label={t("common.search")}
          placeholder={t("common.search")}
        />
      )}
      {groups.length === 0 && <p className="panel-card">{t("duplicates.none")}</p>}
      {groups.length > 0 && shown.length === 0 && <p className="panel-card">{t("duplicates.noMatches")}</p>}
      {shown.map((group) => {
        const id = group.members[0]!.id;
        const kept = keep[id] ?? id;
        return (
          <div className="panel-card duplicate-group" key={id}>
            <h2>
              {group.site}
              {group.username ? ` · ${group.username}` : ""}
            </h2>
            <p className={group.same_password ? "muted" : "warn"}>
              {group.same_password ? t("duplicates.samePassword") : t("duplicates.differentPasswords")}
            </p>
            <ul className="duplicate-members">
              {group.members.map((m, index) => (
                <li key={m.id}>
                  <label>
                    <input
                      type="radio"
                      name={`keep-${id}`}
                      checked={kept === m.id}
                      onChange={() => setKeep({ ...keep, [id]: m.id })}
                    />
                    <span className="duplicate-text">
                      <strong>{m.title}</strong>
                      {index === 0 && <span className="badge">{t("duplicates.suggested")}</span>}
                      <span className="muted">
                        {m.urls.join(", ")}
                        {" · "}
                        {t("duplicates.changed", { date: new Date(m.updated_at).toLocaleDateString() })}
                        {m.has_totp ? ` · ${t("duplicates.hasTotp")}` : ""}
                        {m.has_notes ? ` · ${t("duplicates.hasNotes")}` : ""}
                        {group.same_password ? "" : ` · ${t("duplicates.passwordNumber", { n: m.password_group + 1 })}`}
                      </span>
                    </span>
                  </label>
                  <button type="button" className="link" onClick={() => onOpenItem(m.id)}>
                    {t("duplicates.open")}
                  </button>
                </li>
              ))}
            </ul>
            {!group.same_password && <p className="muted">{t("duplicates.differentHint")}</p>}
            <button type="button" disabled={busy !== null} onClick={() => void merge(group)}>
              {t("duplicates.merge", { n: group.members.length - 1 })}
            </button>
          </div>
        );
      })}
    </section>
  );
}
