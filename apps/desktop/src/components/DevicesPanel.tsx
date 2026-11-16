import { useCallback, useEffect, useState, type FormEvent } from "react";

import { useClient } from "../api/context";
import type { Translate } from "../../../../i18n/translate";
import type { Device, ThisDevice, Vault } from "../api/types";
import { useAction } from "../hooks/useAction";
import { englishT, useT } from "../i18n";
import { rich } from "../i18n/richText";
import { nativeFilePicker, type FilePicker } from "../lib/files";
import { relativeTime } from "../lib/relativeTime";
import { AddDevice } from "./AddDevice";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  vaults: Vault[];
  /** Called after a sync changed items here, so lists refresh. */
  onSynced: () => void;
  onClose: () => void;
  /** Bumps when a background sync finished, to refresh the list. */
  refreshKey?: number;
  files?: FilePicker;
  /** Copies text (the pairing code); the clipboard is cleared again later. */
  copy?: (text: string) => Promise<void>;
}

/** "Password365 sync for Alice's phone.qlsync" style default names, safe for any OS. */
export function syncFileName(deviceName: string, t: Translate = englishT): string {
  const safe = deviceName.replace(/[^\p{L}\p{N} _-]/gu, "").trim() || t("devices.fallbackName");
  return t("devices.fileName", { name: safe });
}

/** Devices: this device's name, paired devices with sync status, add/remove, Sync now. */
export function DevicesPanel({ vaults, onSynced, onClose, refreshKey = 0, files = nativeFilePicker, copy }: Props) {
  const client = useClient();
  const t = useT();
  const [me, setMe] = useState<ThisDevice | null>(null);
  const [name, setName] = useState("");
  const [devices, setDevices] = useState<Device[]>([]);
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    const [self, list] = await Promise.all([client.thisDevice(), client.devices()]);
    setMe(self);
    setName(self.name);
    setDevices(list);
  }, [client]);
  const loading = useAction(load);
  const { run: reload } = loading;
  useEffect(() => {
    void reload();
  }, [reload, refreshKey]);

  const rename = useAction(async (e: FormEvent) => {
    e.preventDefault();
    setMe(await client.renameThisDevice(name));
  });
  const sync = useAction(async (device: Device) => {
    setMessage(null);
    const result = await client.syncDevice(device.id);
    const synced = t("devices.synced", { name: device.name, count: result.changed });
    const conflicts = result.conflicts.length ? ` ${t("devices.conflicts", { count: result.conflicts.length })}` : "";
    setMessage(synced + conflicts);
    if (result.changed > 0) onSynced();
    await load();
  });
  const exportFile = useAction(async (device: Device) => {
    setMessage(null);
    const path = await files.saveSyncFile(syncFileName(device.name, t));
    if (!path) return;
    const count = await client.exportSyncFile(device.id, path);
    setMessage(t("devices.fileSaved", { name: device.name, count }));
  });
  const importFile = useAction(async () => {
    setMessage(null);
    const path = await files.openSyncFile();
    if (!path) return;
    const result = await client.importSyncFile(path);
    setMessage(t("devices.fileImported", { name: result.device.name, count: result.changed }));
    if (result.changed > 0) onSynced();
    await load();
  });
  const remove = useAction(async (id: string) => {
    await client.removeDevice(id);
    setRemoving(null);
    await load();
  });

  const now = Date.now();
  return (
    <article className="devices">
      <header className="detail-header">
        <h1>{t("devices.title")}</h1>
        <button type="button" className="ghost" onClick={onClose}>
          {t("common.close")}
        </button>
      </header>
      <p className="hint">{t("devices.hint")}</p>

      <form onSubmit={rename.run} className="inline-form">
        <label>
          {t("devices.thisName")}
          <input value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <button type="submit" disabled={rename.busy || !name.trim() || name.trim() === me?.name}>
          {t("devices.rename")}
        </button>
      </form>

      <h2>{t("devices.paired")}</h2>
      {devices.length === 0 && <p className="muted">{t("devices.none")}</p>}
      <ul className="device-list">
        {devices.map((d) => (
          <li key={d.id}>
            <span className="device-info">
              <strong>{d.name}</strong>
              <span className="muted">
                {t("devices.lastSynced", { when: relativeTime(d.last_sync, now, t) })}
                {d.pending_changes > 0 && ` · ${t("devices.pending", { count: d.pending_changes })}`}
              </span>
            </span>
            <span className="device-actions">
              <button type="button" disabled={sync.busy} onClick={() => void sync.run(d)}>
                {t("devices.syncNow")}
              </button>
              <button
                type="button"
                className="ghost"
                aria-label={t("devices.saveSyncFile", { name: d.name })}
                onClick={() => void exportFile.run(d)}
              >
                {t("devices.syncFile")}
              </button>
              {removing === d.id ? (
                <>
                  <span className="muted" role="note">
                    {t("devices.removeWarning", { name: d.name })}
                  </span>
                  <button type="button" className="danger" onClick={() => void remove.run(d.id)}>
                    {t("common.remove")}
                  </button>
                  <button type="button" className="ghost" onClick={() => setRemoving(null)}>
                    {t("common.cancel")}
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  className="ghost"
                  aria-label={t("devices.removeNamed", { name: d.name })}
                  onClick={() => setRemoving(d.id)}
                >
                  {t("devices.removeAsk")}
                </button>
              )}
            </span>
          </li>
        ))}
      </ul>
      {message && <p role="status">{message}</p>}
      {devices.length > 0 && (
        <p className="hint">
          {rich(t("devices.noNetwork"), {
            syncFile: <strong>{t("devices.syncFile")}</strong>,
            importLink: (
              <button type="button" className="link" onClick={() => void importFile.run()}>
                {t("devices.importLink")}
              </button>
            ),
          })}
        </p>
      )}
      <ErrorMessage
        error={loading.error ?? rename.error ?? sync.error ?? remove.error ?? exportFile.error ?? importFile.error}
      />

      {adding ? (
        <AddDevice
          vaults={vaults}
          copy={copy}
          onAdded={(device) => {
            setAdding(false);
            setMessage(t("devices.added", { name: device.name }));
            void load();
          }}
          onClose={() => setAdding(false)}
        />
      ) : (
        <button type="button" onClick={() => setAdding(true)}>
          {t("devices.add")}
        </button>
      )}
    </article>
  );
}
