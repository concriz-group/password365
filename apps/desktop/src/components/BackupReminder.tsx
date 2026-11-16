import { useBackup } from "../hooks/useBackup";
import type { FilePicker } from "../lib/files";
import { ErrorMessage } from "./ErrorMessage";
import { useT } from "../i18n";

/** Banner shown when the vault has items and no backup in 30 days. */
export function BackupReminder({ files, hasItems }: { files: FilePicker; hasItems: boolean }) {
  const t = useT();
  const { info, backUp } = useBackup(files);
  if (!hasItems || !info?.due) return null;
  return (
    <div className="banner" role="note">
      <span>
        {info.last_backup_ms ? t("backupReminder.stale") : t("backupReminder.none")} {t("backupReminder.risk")}
      </span>
      <button type="button" disabled={backUp.busy} onClick={() => void backUp.run()}>
        {t("backupReminder.backUp")}
      </button>
      <ErrorMessage error={backUp.error} />
    </div>
  );
}
