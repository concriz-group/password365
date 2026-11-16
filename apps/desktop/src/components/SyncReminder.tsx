import { useEffect, useState } from "react";

import { useClient } from "../api/context";
import type { Device } from "../api/types";
import { useT } from "../i18n";

interface Props {
  /** Opens the Devices screen. */
  onOpenDevices: () => void;
  /** Changes when a sync finished, to re-check. */
  refreshKey?: number;
}

/**
 * Banner shown while a newly paired device has not synced yet, or when a
 * paired device has been missing changes for a long time.
 */
export function SyncReminder({ onOpenDevices, refreshKey = 0 }: Props) {
  const client = useClient();
  const t = useT();
  const [devices, setDevices] = useState<Device[]>([]);

  useEffect(() => {
    let alive = true;
    client.devices().then(
      (list) => alive && setDevices(list),
      () => alive && setDevices([]),
    );
    return () => {
      alive = false;
    };
  }, [client, refreshKey]);

  const fresh = devices.find((d) => d.last_sync === null);
  if (fresh) {
    return (
      <div className="banner" role="status">
        <span>{t("syncReminder.firstSync", { name: fresh.name })}</span>
      </div>
    );
  }
  const behind = devices.filter((d) => d.out_of_sync);
  const first = behind[0];
  if (!first) return null;
  const names = behind.length > 1 ? t("syncReminder.more", { name: first.name, count: behind.length - 1 }) : first.name;
  return (
    <div className="banner" role="note">
      <span>{t("syncReminder.missing", { devices: names, count: first.pending_changes })}</span>
      <button type="button" onClick={onOpenDevices}>
        {t("syncReminder.devices")}
      </button>
    </div>
  );
}
