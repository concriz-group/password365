import { useEffect, useState } from "react";

import type { Item, Vault } from "../api/types";
import { useClient } from "../api/context";
import { categoryLabel, useCategories } from "../hooks/useCategories";
import { DEFAULT_SEQUENCE, SEQUENCE_LABEL, WINDOW_LABEL, isAutoTypeField } from "../lib/autoType";
import { grouped, specOf } from "../lib/fieldSections";
import { formatDate } from "../lib/format";
import { useT } from "../i18n";
import { translateOr } from "../../../../i18n/translate";
import { ErrorMessage } from "./ErrorMessage";
import { ItemIcon } from "./ItemIcon";
import { nativeFilePicker, type FilePicker } from "../lib/files";
import { AttachmentsSection } from "./AttachmentsSection";
import { ItemMenu } from "./ItemMenu";
import { SignInWithRow } from "./SignInWithRow";
import { WifiQr } from "./WifiQr";
import { SecretRow } from "./SecretRow";
import { TotpRow } from "./TotpRow";

interface Props {
  /** File dialogs (attachments). */
  files?: FilePicker;
  /** Vaults for Move… in the item menu. */
  vaults?: Vault[];
  /** Opens another item (the copy made by Duplicate). */
  onOpen?: (id: string) => void;
  id: string;
  vaultName: (vaultId: string) => string;
  onEdit: (item: Item) => void;
  onDeleted: () => void;
  /** The item moved into or out of the Archive (it leaves the current list). */
  onArchived?: () => void;
  copy: (text: string) => Promise<void>;
  /** Called after a change the item list shows (review dismissed, favorite toggled). */
  onChanged?: () => void;
}

/** A visible value with a Copy button. */
function PlainRow({ label, value, copy }: { label: string; value: string; copy: (text: string) => Promise<void> }) {
  const t = useT();
  return (
    <div className="detail-row">
      <span className="detail-label">{label}</span>
      <span className="detail-value pre">{value}</span>
      <span className="detail-actions">
        <button type="button" className="ghost" onClick={() => void copy(value)}>
          {t("common.copy")}
        </button>
      </span>
    </div>
  );
}

/** The auto-type sequence and windows, and the button that types into the previous window. */
function AutoTypeRow({ sequence, windows, onAutoType }: { sequence?: string | null; windows?: string | null; onAutoType: () => void }) {
  const t = useT();
  return (
    <div className="detail-row">
      <span className="detail-label">{t("itemDetail.autoType")}</span>
      <span className="detail-value pre muted small">
        {sequence || DEFAULT_SEQUENCE}
        {windows && `\n${t("itemDetail.autoTypeWindows", { windows: windows.split("\n").join(", ") })}`}
      </span>
      <span className="detail-actions">
        <button type="button" className="ghost" title={t("itemDetail.autoTypeTitle")} onClick={onAutoType}>
          {t("itemDetail.autoType")}
        </button>
      </span>
    </div>
  );
}

/** The login's passkey: the site and account it signs in to (the key never leaves the app). */
function PasskeyRow({ passkey, onRemove }: { passkey: { rp_id: string; user_name: string }; onRemove: () => void }) {
  const t = useT();
  const [confirming, setConfirming] = useState(false);
  return (
    <div className="detail-row">
      <span className="detail-label">{t("itemDetail.passkey")}</span>
      <span className="detail-value">{t("itemDetail.passkeyFor", { site: passkey.rp_id, user: passkey.user_name })}</span>
      <span className="detail-actions">
        {confirming ? (
          <>
            <button type="button" className="danger" onClick={onRemove}>
              {t("itemDetail.removePasskey")}
            </button>
            <button type="button" className="ghost" onClick={() => setConfirming(false)}>
              {t("common.cancel")}
            </button>
          </>
        ) : (
          <button type="button" className="ghost" onClick={() => setConfirming(true)}>
            {t("common.remove")}
          </button>
        )}
      </span>
    </div>
  );
}

function StarIcon() {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
      <path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z" />
    </svg>
  );
}

/** Read-only view of one item. */
export function ItemDetail({ id, vaultName, onEdit, onDeleted, onArchived, copy, onChanged, vaults = [], onOpen, files = nativeFilePicker }: Props) {
  const client = useClient();
  const t = useT();
  const categories = useCategories();
  const [item, setItem] = useState<Item | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEffect(() => {
    setItem(null);
    setConfirmDelete(false);
    client.item(id).then(setItem, setError);
  }, [client, id]);

  /** Reloads the item after a change made here (a file attached or removed). */
  const reload = () => {
    client.item(id).then(setItem, setError);
    onChanged?.();
  };

  async function remove() {
    try {
      await client.deleteItem(id);
      onDeleted();
    } catch (e) {
      setError(e);
    }
  }

  async function reviewed() {
    try {
      await client.markReviewed(id);
      setItem((current) => current && { ...current, needs_review: false });
      onChanged?.();
    } catch (e) {
      setError(e);
    }
  }

  async function archive(archived: boolean) {
    try {
      await client.setArchived(id, archived);
      onArchived?.();
    } catch (e) {
      setError(e);
    }
  }

  async function autoType() {
    try {
      await client.autoType(id);
    } catch (e) {
      setError(e);
    }
  }

  async function move(vaultId: string) {
    try {
      setItem(await client.moveItem(id, vaultId));
      onChanged?.();
    } catch (e) {
      setError(e);
    }
  }

  async function duplicate() {
    if (!item) return;
    try {
      const copy = await client.duplicateItem(id, t("itemMenu.copyTitle", { title: item.title }));
      onChanged?.();
      onOpen?.(copy.id);
    } catch (e) {
      setError(e);
    }
  }

  async function toggleFavorite(favorite: boolean) {
    try {
      await client.setFavorite(id, favorite);
      setItem((current) => current && { ...current, favorite });
      onChanged?.();
    } catch (e) {
      setError(e);
    }
  }

  if (error) return <ErrorMessage error={error} />;
  if (!item) return <p className="muted">{t("common.loading")}</p>;
  const category = categories.find((c) => c.kind === item.kind);
  const fieldLabel = (label: string) =>
    translateOr(t, `fields.${specOf(category, item.region ?? "", label)?.name ?? label}`, label);

  return (
    <article className="item-detail" aria-label={item.title}>
      <header>
        <h1>
          <ItemIcon title={item.title} icon={item.icon} kind={item.kind} large />
          {item.title}
          <button
            type="button"
            className={item.favorite ? "star on" : "star"}
            aria-label={t("itemDetail.favorite")}
            aria-pressed={item.favorite}
            title={t(item.favorite ? "itemDetail.removeFavorite" : "itemDetail.addFavorite")}
            onClick={() => void toggleFavorite(!item.favorite)}
          >
            <StarIcon />
          </button>
        </h1>
        <span className="muted">
          {translateOr(t, `categories.${item.kind}`, categoryLabel(categories, item.kind))} · {vaultName(item.vault_id)}
        </span>
        <ItemMenu
          item={item}
          vaults={vaults}
          onFavorite={(f) => void toggleFavorite(f)}
          onMove={(v) => void move(v)}
          onDuplicate={() => void duplicate()}
          onArchive={() => void archive(true)}
          onDelete={() => setConfirmDelete(true)}
        />
      </header>
      {item.archived && (
        <div className="notice" role="note">
          <p>{t("itemDetail.archivedNotice")}</p>
          <button type="button" onClick={() => void archive(false)}>
            {t("itemDetail.restore")}
          </button>
        </div>
      )}
      {item.needs_review && (
        <div className="notice" role="note">
          <p>{t("itemDetail.reviewNotice")}</p>
          <button type="button" onClick={() => void reviewed()}>
            {t("common.done")}
          </button>
        </div>
      )}
      {item.username && <PlainRow label={t("itemDetail.username")} value={item.username} copy={copy} />}
      {item.has_password && (
        <SecretRow label={t("itemDetail.password")} reveal={() => client.reveal(id, "password")} copy={copy} />
      )}
      {item.has_totp && <TotpRow itemId={id} copy={copy} />}
      {item.passkey && <PasskeyRow passkey={item.passkey} onRemove={() => void client.removePasskey(id).then(reload, setError)} />}
      {item.urls.map((url) => (
        <div className="detail-row" key={url}>
          <span className="detail-label">{t("itemDetail.website")}</span>
          <span className="detail-value">{url}</span>
        </div>
      ))}
      {grouped(
        item.fields.filter((f) => !isAutoTypeField(f.label)),
        (f) => f.section,
      ).map(([section, fields]) => (
        <div
          className="detail-section"
          key={section}
          role="group"
          aria-label={section ? translateOr(t, `sections.${section}`, section) : undefined}
        >
          {section && <h3 className="section-title">{translateOr(t, `sections.${section}`, section)}</h3>}
          {fields.map((f) =>
            f.input === "sign_in_with" && f.value ? (
              <SignInWithRow key={f.label} label={fieldLabel(f.label)} itemId={f.value} onOpen={onOpen} />
            ) : f.input === "totp" ? (
              <TotpRow key={f.label} itemId={id} label={f.label} title={fieldLabel(f.label)} copy={copy} />
            ) : f.concealed ? (
              <SecretRow key={f.label} label={fieldLabel(f.label)} reveal={() => client.revealField(id, f.label)} copy={copy} />
            ) : (
              <PlainRow key={f.label} label={fieldLabel(f.label)} value={f.value ?? ""} copy={copy} />
            ),
          )}
        </div>
      ))}
      {/* Without a password (a passkey-only login, say) the default sequence makes no sense. */}
      {(item.has_password || item.fields.some((f) => f.label === SEQUENCE_LABEL)) && (
        <AutoTypeRow
          sequence={item.fields.find((f) => f.label === SEQUENCE_LABEL)?.value}
          windows={item.fields.find((f) => f.label === WINDOW_LABEL)?.value}
          onAutoType={() => void autoType()}
        />
      )}
      {item.kind === "wireless_router" && <WifiQr itemId={id} />}
      <AttachmentsSection itemId={id} attachments={item.attachments ?? []} files={files} onChanged={reload} />
      {item.has_notes && (
        <SecretRow label={t("itemDetail.notes")} reveal={() => client.reveal(id, "notes")} copy={copy} multiline />
      )}
      {item.tags.length > 0 && (
        <div className="detail-row">
          <span className="detail-label">{t("itemDetail.tags")}</span>
          <span className="detail-value">{item.tags.join(", ")}</span>
        </div>
      )}
      <p className="muted small">
        {t("itemDetail.modified", { date: formatDate(item.updated_at) })}
        {(item.has_password || item.password_history_count > 0) &&
          ` · ${t("itemDetail.previousPasswords", { count: item.password_history_count })}`}
      </p>
      <footer className="detail-footer">
        <button type="button" onClick={() => onEdit(item)}>
          {t("common.edit")}
        </button>
        {!item.archived && (
          <button type="button" className="ghost" onClick={() => void archive(true)}>
            {t("itemDetail.archive")}
          </button>
        )}
        {confirmDelete ? (
          <>
            <button type="button" className="danger" onClick={remove}>
              {t("itemDetail.moveToDeleted")}
            </button>
            <button type="button" className="ghost" onClick={() => setConfirmDelete(false)}>
              {t("common.cancel")}
            </button>
          </>
        ) : (
          <button type="button" className="ghost" onClick={() => setConfirmDelete(true)}>
            {t("common.delete")}
          </button>
        )}
      </footer>
    </article>
  );
}
