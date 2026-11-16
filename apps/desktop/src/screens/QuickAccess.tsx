import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";

import type { AppStatus, ItemSummary } from "../api/types";
import { useClient } from "../api/context";
import type { StatusSubscription } from "../App";
import { ErrorMessage } from "../components/ErrorMessage";
import { useT } from "../i18n";
import { useSettings } from "../settings";

const SHOWN = 8;
const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

type Copyable = "username" | "password" | "totp";

/**
 * Quick Access, like 1Password's: search from any program, copy a username,
 * password or one-time password, or open the item in the app. Runs in its own
 * small window; it hides after a copy, on Escape or when it loses focus.
 */
export function QuickAccess({ subscribeShown, copy: injectedCopy }: { subscribeShown?: StatusSubscription; copy?: (text: string) => Promise<void> }) {
  const client = useClient();
  const t = useT();
  const { settings } = useSettings();
  const copy = injectedCopy ?? ((text: string) => client.copySecret(text, settings.clipboard_clear_seconds * 1000));
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [selected, setSelected] = useState(0);
  const [error, setError] = useState<unknown>(null);
  const input = useRef<HTMLInputElement>(null);

  const refresh = useCallback(() => {
    setQuery("");
    setSelected(0);
    setError(null);
    client.status().then(setStatus, setError);
    input.current?.focus();
  }, [client]);
  useEffect(refresh, [refresh]);
  useEffect(() => subscribeShown?.(refresh), [subscribeShown, refresh]);

  useEffect(() => {
    if (!status?.unlocked) return;
    let live = true;
    client.items(query, undefined).then(
      (found) => live && setItems(found.slice(0, SHOWN)),
      (e) => live && setError(e),
    );
    return () => {
      live = false;
    };
  }, [client, query, status]);

  useEffect(() => {
    const hide = () => void client.hideQuickAccess().catch(() => undefined);
    window.addEventListener("blur", hide);
    return () => window.removeEventListener("blur", hide);
  }, [client]);

  const current = items[selected];

  async function copyField(item: ItemSummary | undefined, field: Copyable) {
    if (!item) return;
    try {
      const value =
        field === "username"
          ? item.username
          : field === "password"
            ? await client.reveal(item.id, "password")
            : (await client.totpCode(item.id))?.code;
      if (!value) {
        setError(new Error(t(`quick.no_${field}`)));
        return;
      }
      await copy(value);
      await client.hideQuickAccess();
    } catch (e) {
      setError(e);
    }
  }

  const open = (id: string | null) => void client.openInApp(id).catch(setError);

  function onKey(e: KeyboardEvent) {
    const command = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === "Escape") void client.hideQuickAccess();
    else if (e.key === "ArrowDown") setSelected((s) => Math.min(items.length - 1, s + 1));
    else if (e.key === "ArrowUp") setSelected((s) => Math.max(0, s - 1));
    else if (e.key === "Enter") void copyField(current, "password");
    else if (command && e.key.toLowerCase() === "c" && (e.shiftKey || e.altKey || !window.getSelection()?.toString())) {
      void copyField(current, e.altKey ? "totp" : e.shiftKey ? "password" : "username");
    } else if (command && e.key.toLowerCase() === "o" && current) open(current.id);
    else return;
    e.preventDefault();
  }

  if (status && !status.unlocked) {
    return (
      <section className="quick-access" aria-label={t("quick.title")} onKeyDown={onKey}>
        <p>{t("quick.locked")}</p>
        <button type="button" className="primary" autoFocus onClick={() => open(null)}>
          {t("quick.unlock")}
        </button>
      </section>
    );
  }

  const key = isMac ? "⌘" : "Ctrl+";
  return (
    <section className="quick-access" aria-label={t("quick.title")} onKeyDown={onKey}>
      <input
        ref={input}
        className="quick-search"
        type="search"
        autoFocus
        aria-label={t("quick.search")}
        placeholder={t("quick.search")}
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setSelected(0);
        }}
      />
      <ErrorMessage error={error} />
      <ul className="quick-results" role="listbox" aria-label={t("quick.results")}>
        {items.map((item, i) => (
          <li
            key={item.id}
            role="option"
            aria-selected={i === selected}
            className={i === selected ? "selected" : undefined}
            onMouseEnter={() => setSelected(i)}
            onDoubleClick={() => open(item.id)}
          >
            <span className="row-title">{item.title}</span>
            <span className="muted">{item.username ?? ""}</span>
          </li>
        ))}
        {status?.unlocked && items.length === 0 && <li className="muted">{t("quick.none")}</li>}
      </ul>
      {current && (
        <footer className="quick-actions">
          <button type="button" onClick={() => void copyField(current, "username")}>
            {t("quick.copyUsername")} <kbd>{key}C</kbd>
          </button>
          <button type="button" onClick={() => void copyField(current, "password")}>
            {t("quick.copyPassword")} <kbd>↵</kbd>
          </button>
          <button type="button" onClick={() => void copyField(current, "totp")}>
            {t("quick.copyTotp")} <kbd>{isMac ? "⌥⌘C" : "Ctrl+Alt+C"}</kbd>
          </button>
          <button type="button" className="ghost" onClick={() => open(current.id)}>
            {t("quick.open")} <kbd>{key}O</kbd>
          </button>
        </footer>
      )}
    </section>
  );
}
