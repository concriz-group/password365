import { useCallback, useEffect, useState } from "react";

import type { ItemSummary, Profile, ProfileItem, ProfileKind } from "../api/types";
import { useClient } from "../api/context";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  /** Opens the editor for a new item of this category. */
  onCreate: (kind: string) => void;
}

/** Each slot's texts are `profile.<slot>Title`, `<slot>Text`, `<slot>Label`. */
const SLOTS: { kind: ProfileKind; itemKind: string; create: string }[] = [
  { kind: "identity", itemKind: "identity", create: "profile.createIdentity" },
  { kind: "card", itemKind: "card", create: "profile.addCard" },
];

/** Profile, like 1Password's: the identity and card the browser extension fills into forms. */
export function ProfilePanel({ onCreate }: Props) {
  const client = useClient();
  const t = useT();
  const [profile, setProfile] = useState<Profile | null>(null);
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [error, setError] = useState<unknown>(null);

  const load = useCallback(async () => {
    try {
      const [p, all] = await Promise.all([client.profile(), client.items("", undefined)]);
      setProfile(p);
      setItems(all);
    } catch (e) {
      setError(e);
    }
  }, [client]);

  useEffect(() => {
    void load();
  }, [load]);

  async function choose(kind: ProfileKind, id: string) {
    try {
      await client.setProfile(kind, id || null);
      await load();
    } catch (e) {
      setError(e);
    }
  }

  if (!profile) return error ? <ErrorMessage error={error} /> : <p className="muted">{t("common.loading")}</p>;

  const describe = (item: ProfileItem | null) => (item ? `${item.title}${item.last4 ? ` •••• ${item.last4}` : ""}` : "");

  return (
    <section className="profile" aria-label={t("profile.title")}>
      <h1>{t("profile.title")}</h1>
      <div className="notice">
        <p>
          <strong>{t("profile.noticeStrong")}</strong> {t("profile.noticeText")}
        </p>
      </div>
      <div className="finding-cards">
        {SLOTS.map((slot) => {
          const options = items.filter((i) => i.kind === slot.itemKind);
          const current = profile[slot.kind];
          return (
            <div className="panel-card profile-slot" key={slot.kind}>
              <h2>{t(`profile.${slot.kind}Title`)}</h2>
              <p className="muted">{t(`profile.${slot.kind}Text`)}</p>
              {options.length === 0 ? (
                <button type="button" onClick={() => onCreate(slot.itemKind)}>
                  {t(slot.create)}
                </button>
              ) : (
                <div className="field">
                  <label htmlFor={`profile-${slot.kind}`}>{t(`profile.${slot.kind}Label`)}</label>
                  <select
                    id={`profile-${slot.kind}`}
                    value={current?.id ?? ""}
                    onChange={(e) => void choose(slot.kind, e.target.value)}
                  >
                    <option value="">{t("common.none")}</option>
                    {options.map((o) => (
                      <option key={o.id} value={o.id}>
                        {o.id === current?.id ? describe(current) : o.title}
                      </option>
                    ))}
                  </select>
                </div>
              )}
            </div>
          );
        })}
      </div>
      <ErrorMessage error={error} />
    </section>
  );
}
