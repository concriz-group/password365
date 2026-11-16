import { useEffect, useState } from "react";

import { useClient } from "../api/context";
import { useT } from "../i18n";

/** A "Sign in with" field: the login it points at, opened with a click. */
export function SignInWithRow({ label, itemId, onOpen }: { label: string; itemId: string; onOpen?: (id: string) => void }) {
  const client = useClient();
  const t = useT();
  // undefined: loading; null: the login no longer exists.
  const [title, setTitle] = useState<string | null | undefined>(undefined);
  useEffect(() => {
    let live = true;
    client.item(itemId).then(
      (found) => live && setTitle(found?.title ?? null),
      () => live && setTitle(null),
    );
    return () => {
      live = false;
    };
  }, [client, itemId]);
  return (
    <div className="detail-row">
      <span className="detail-label">{label}</span>
      <span className="detail-value">
        {title === undefined ? null : title === null ? (
          <span className="muted">{t("itemDetail.missingLogin")}</span>
        ) : (
          <button type="button" className="link" onClick={() => onOpen?.(itemId)}>
            {title}
          </button>
        )}
      </span>
      <span className="detail-actions" />
    </div>
  );
}
