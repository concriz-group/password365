import { useEffect, useRef, useState } from "react";

import { PaneResizer, useListWidth } from "../components/PaneResizer";
import { BrandLockup } from "../components/BrandMark";
import type { Subscription } from "../App";
import type { AutoTypeChoice, Item, SshApproval, UpdateInfo } from "../api/types";
import { AutoTypeChooser } from "../components/AutoTypeChooser";
import { useClient } from "../api/context";
import { BackupReminder } from "../components/BackupReminder";
import { DevicesPanel } from "../components/DevicesPanel";
import { ErrorMessage } from "../components/ErrorMessage";
import { Generator } from "../components/Generator";
import { ItemDetail } from "../components/ItemDetail";
import { ItemEditor } from "../components/ItemEditor";
import { ItemList } from "../components/ItemList";
import { NewItemPicker } from "../components/NewItemPicker";
import { ProfilePanel } from "../components/ProfilePanel";
import { WatchtowerPanel } from "../components/WatchtowerPanel";
import { useCategories } from "../hooks/useCategories";
import { RecentlyDeleted } from "../components/RecentlyDeleted";
import { SettingsPanel } from "../components/SettingsPanel";
import { Sidebar } from "../components/Sidebar";
import { PasswordUpdateBanner } from "../components/PasswordUpdateBanner";
import { SyncReminder } from "../components/SyncReminder";
import { UpdateBanner } from "../components/UpdateBanner";
import { Transfer } from "../components/Transfer";
import { useVaultData } from "../hooks/useVaultData";
import { useSettings } from "../settings";
import { SshApprovalPrompt } from "../components/SshApprovalPrompt";
import { nativeFilePicker, type FilePicker } from "../lib/files";
import { useT } from "../i18n";

/** What the right-hand pane shows. */
type Pane =
  | { kind: "empty" }
  | { kind: "view"; id: string }
  | { kind: "pick" }
  | { kind: "edit"; item: Item | null; newKind?: string }
  | { kind: "generator" }
  | { kind: "transfer" }
  | { kind: "settings" }
  | { kind: "deleted" }
  | { kind: "watchtower" }
  | { kind: "profile" }
  | { kind: "autotype"; choice: AutoTypeChoice }
  | { kind: "devices" };

interface Props {
  onLocked: () => void;
  /** Registers for "a background sync changed items"; returns an unsubscribe function. */
  subscribeSync?: (onSynced: () => void) => () => void;
  /** Registers for "the auto-type shortcut needs a choice"; returns an unsubscribe function. */
  subscribeAutoType?: Subscription<AutoTypeChoice>;
  copy?: (text: string) => Promise<void>;
  files?: FilePicker;
  /** Quick Access asked to show an item here. */
  subscribeOpenItem?: Subscription<{ id: string | null }>;
  /** A program wants to sign with an SSH key. */
  subscribeSshApprove?: Subscription<SshApproval>;
  /** An automatic check found a new version. */
  subscribeUpdate?: Subscription<UpdateInfo>;
}

/** Unlocked vault: sidebar · item list · detail/editor/generator. */
export function Main({ onLocked, subscribeSync, subscribeAutoType, subscribeOpenItem, subscribeSshApprove, subscribeUpdate, copy: injectedCopy, files = nativeFilePicker }: Props) {
  const [listWidth, setListWidth] = useListWidth();
  const client = useClient();
  const t = useT();
  const { settings, loaded } = useSettings();
  const copy =
    injectedCopy ?? ((text: string) => client.copySecret(text, settings.clipboard_clear_seconds * 1000));
  const data = useVaultData(client);
  const categories = useCategories();
  const [syncs, setSyncs] = useState(0);
  const { reload } = data;
  useEffect(
    () =>
      subscribeSync?.(() => {
        void reload();
        setSyncs((n) => n + 1);
      }),
    [subscribeSync, reload],
  );
  const [pane, setPane] = useState<Pane>({ kind: "empty" });
  // Settings › General › Open Password365 to: once, when the vault opens (not when the setting changes later).
  const opened = useRef(false);
  const { setScope } = data;
  useEffect(() => {
    if (!loaded || opened.current) return;
    opened.current = true;
    if (settings.open_to === "favorites") setScope({ type: "favorites" });
    if (settings.open_to === "watchtower") setPane({ kind: "watchtower" });
  }, [loaded, settings.open_to, setScope]);
  // Items' own icons for the list: refreshed whenever the list is.
  const [icons, setIcons] = useState<Record<string, string>>({});
  useEffect(() => {
    let live = true;
    client.itemIcons().then(
      (found) => live && setIcons(found ?? {}),
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [client, data.items]);
  const defaultVault = data.vaults.some((v) => v.id === settings.default_vault) ? settings.default_vault : null;
  useEffect(() => subscribeAutoType?.((choice) => setPane({ kind: "autotype", choice })), [subscribeAutoType]);
  useEffect(() => subscribeOpenItem?.(({ id }) => id && setPane({ kind: "view", id })), [subscribeOpenItem]);
  const [sshRequests, setSshRequests] = useState<SshApproval[]>([]);
  useEffect(() => subscribeSshApprove?.((request) => setSshRequests((all) => [...all, request])), [subscribeSshApprove]);
  const [error, setError] = useState<unknown>(null);

  const vaultName = (id: string) => data.vaults.find((v) => v.id === id)?.name ?? "";
  const selectedId = pane.kind === "view" ? pane.id : null;
  const close = () => setPane({ kind: "empty" });

  async function lock() {
    await client.lock();
    onLocked();
  }

  async function createVault(name: string) {
    try {
      const vault = await client.createVault(name);
      await data.reload();
      data.setVaultId(vault.id);
    } catch (e) {
      setError(e);
    }
  }

  async function saved(item: Item) {
    await data.reload();
    setPane({ kind: "view", id: item.id });
  }

  /** The open item left the current list (deleted, archived or restored). */
  async function deleted() {
    await data.reload();
    setPane({ kind: "empty" });
  }

  return (
    <main className="app">
      {sshRequests[0] && (
        <SshApprovalPrompt
          key={sshRequests[0].id}
          request={sshRequests[0]}
          onDone={() => setSshRequests((all) => all.slice(1))}
        />
      )}
      <header className="toolbar">
        <BrandLockup layout="inline" />
        <span className="toolbar-actions">
          <button type="button" onClick={() => setPane({ kind: "pick" })}>
            {t("main.newItem")}
          </button>
          <button type="button" onClick={() => setPane({ kind: "generator" })}>
            {t("main.generator")}
          </button>
          <button type="button" onClick={() => setPane({ kind: "transfer" })}>
            {t("main.transfer")}
          </button>
          <button type="button" onClick={() => setPane({ kind: "devices" })}>
            {t("main.devices")}
          </button>
          <button type="button" onClick={() => setPane({ kind: "settings" })}>
            {t("main.settings")}
          </button>
          <button type="button" onClick={lock}>
            {t("main.lock")}
          </button>
        </span>
      </header>
      <UpdateBanner subscribeUpdate={subscribeUpdate} />
      <BackupReminder files={files} hasItems={data.vaults.some((v) => v.item_count > 0)} />
      <PasswordUpdateBanner refreshKey={syncs} />
      <SyncReminder onOpenDevices={() => setPane({ kind: "devices" })} refreshKey={syncs} />
      <ErrorMessage error={error ?? data.error} />
      <div className="panes" style={{ "--list-width": `${listWidth}px` } as React.CSSProperties}>
        <Sidebar
          vaults={data.vaults}
          tags={settings.sidebar_tags ? data.tags : []}
          categories={settings.sidebar_categories ? data.categories : []}
          category={data.category}
          onCategory={(kind) => {
            data.setCategory(kind);
            if (pane.kind === "deleted" || pane.kind === "watchtower" || pane.kind === "profile") close();
          }}
          selected={data.scope}
          onSelect={(scope) => {
            data.setScope(scope);
            if (pane.kind === "deleted" || pane.kind === "watchtower" || pane.kind === "profile") close();
          }}
          onCreate={createVault}
          showingDeleted={pane.kind === "deleted"}
          onShowDeleted={() => setPane({ kind: "deleted" })}
          showingWatchtower={pane.kind === "watchtower"}
          onShowWatchtower={() => setPane({ kind: "watchtower" })}
          showingProfile={pane.kind === "profile"}
          onShowProfile={() => setPane({ kind: "profile" })}
        />
        <ItemList
          items={data.items}
          icons={icons}
          selectedId={selectedId}
          onSelect={(id) => setPane({ kind: "view", id })}
          query={data.query}
          onQuery={data.setQuery}
          categories={data.categories}
          category={data.category}
          onCategory={data.setCategory}
          sort={data.sort}
          onSort={data.setSort}
        />
        <PaneResizer width={listWidth} onChange={setListWidth} />
        <section className="detail-pane">
          {pane.kind === "empty" && <p className="muted empty">{t("main.empty")}</p>}
          {pane.kind === "view" && (
            <ItemDetail
              id={pane.id}
              vaultName={vaultName}
              onEdit={(item) => setPane({ kind: "edit", item })}
              onDeleted={deleted}
              onArchived={deleted}
              copy={copy}
              onChanged={() => void data.reload()}
              vaults={data.vaults}
              onOpen={(id) => setPane({ kind: "view", id })}
              files={files}
            />
          )}
          {pane.kind === "pick" && (
            <NewItemPicker
              categories={categories}
              onPick={(kind) => setPane({ kind: "edit", item: null, newKind: kind })}
              onCancel={close}
            />
          )}
          {pane.kind === "edit" && (
            <ItemEditor
              key={pane.item?.id ?? `new-${pane.newKind}`}
              item={pane.item}
              kind={pane.newKind}
              vaults={data.vaults}
              defaultVaultId={data.vaultId ?? defaultVault}
              onSaved={saved}
              files={files}
              onCancel={() => setPane(pane.item ? { kind: "view", id: pane.item.id } : { kind: "empty" })}
            />
          )}
          {pane.kind === "generator" && <Generator copy={copy} onClose={close} />}
          {pane.kind === "transfer" && (
            <Transfer vaults={data.vaults} files={files} onImported={() => void data.reload()} onClose={close} />
          )}
          {pane.kind === "settings" && <SettingsPanel onClose={close} files={files} vaults={data.vaults} />}
          {pane.kind === "autotype" && <AutoTypeChooser choice={pane.choice} onDone={close} />}
          {pane.kind === "profile" && (
            <ProfilePanel onCreate={(kind) => setPane({ kind: "edit", item: null, newKind: kind })} />
          )}
          {pane.kind === "watchtower" && (
            <WatchtowerPanel onOpenItem={(id) => setPane({ kind: "view", id })} refreshKey={syncs} />
          )}
          {pane.kind === "deleted" && <RecentlyDeleted onRestored={() => void data.reload()} />}
          {pane.kind === "devices" && (
            <DevicesPanel
              vaults={data.vaults}
              onSynced={() => void data.reload()}
              onClose={close}
              refreshKey={syncs}
              files={files}
              copy={copy}
            />
          )}
        </section>
      </div>
    </main>
  );
}
