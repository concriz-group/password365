import { useCallback, useEffect, useState } from "react";

import { useClient } from "../api/context";
import type { BackupInfo } from "../api/types";
import { backupFileName } from "../lib/backupName";
import type { FilePicker } from "../lib/files";
import { useAction } from "./useAction";

/** Backup status + "Back up now" (native save dialog → encrypted export). */
export function useBackup(files: FilePicker) {
  const client = useClient();
  const [info, setInfo] = useState<BackupInfo | null>(null);

  const refresh = useCallback(() => {
    client.backupInfo().then(setInfo, () => undefined);
  }, [client]);
  useEffect(refresh, [refresh]);

  const backUp = useAction(async () => {
    const path = await files.saveBackup(backupFileName());
    if (!path) return false;
    setInfo(await client.exportBackup(path));
    return true;
  });

  return { info, backUp };
}
