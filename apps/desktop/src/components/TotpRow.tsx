import { useEffect, useState } from "react";

import type { TotpCode } from "../api/types";
import { useClient } from "../api/context";
import { useT } from "../i18n";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  itemId: string;
  /** A one-time password field of the item; omitted: the item's own one-time password. */
  label?: string;
  /** What to show as the field's name (the translated `label`). */
  title?: string;
  copy: (text: string) => Promise<void>;
}

/** "123 456": the code split in two halves, as authenticator apps show it. */
export function formatCode(code: string): string {
  const half = Math.ceil(code.length / 2);
  return `${code.slice(0, half)} ${code.slice(half)}`;
}

/**
 * The item's current one-time password, refreshed when it expires, with a
 * countdown. The secret itself never reaches the window.
 */
export function TotpRow({ itemId, label, title, copy }: Props) {
  const client = useClient();
  const t = useT();
  const name = title ?? label ?? t("secret.totp");
  const [current, setCurrent] = useState<TotpCode | null>(null);
  const [left, setLeft] = useState(0);
  const [error, setError] = useState<unknown>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function refresh() {
      try {
        const next = await (label ? client.totpCode(itemId, label) : client.totpCode(itemId));
        if (!alive || !next) return;
        setCurrent(next);
        setLeft(next.remaining);
        setCopied(false);
        timer = setTimeout(() => void refresh(), next.remaining * 1000);
      } catch (e) {
        if (alive) setError(e);
      }
    }
    void refresh();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [client, itemId, label]);

  useEffect(() => {
    const tick = setInterval(() => setLeft((s) => Math.max(0, s - 1)), 1000);
    return () => clearInterval(tick);
  }, []);

  async function copyCode() {
    if (!current) return;
    await copy(current.code);
    setCopied(true);
  }

  return (
    <div className="detail-row">
      <span className="detail-label">{name}</span>
      <span className="detail-value secret" aria-label={name}>
        {current ? formatCode(current.code) : "…"}
        {current && (
          <span className="muted totp-left" aria-label={t("secret.secondsLeft")}>
            {" "}
            {left}s
          </span>
        )}
      </span>
      <span className="detail-actions">
        <button type="button" className="ghost" onClick={() => void copyCode()} disabled={!current}>
          {t(copied ? "common.copied" : "common.copy")}
        </button>
      </span>
      <ErrorMessage error={error} />
    </div>
  );
}
