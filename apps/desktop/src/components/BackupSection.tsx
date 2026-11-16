import { useState } from "react";

import { useBackup } from "../hooks/useBackup";
import { useI18n } from "../i18n";
import { rich } from "../i18n/richText";
import type { FilePicker } from "../lib/files";
import { ErrorMessage } from "./ErrorMessage";

/** Settings › Backup: last backup date and "Back up now…". */
export function BackupSection({ files }: { files: FilePicker }) {
  const { t, language } = useI18n();
  const { info, backUp } = useBackup(files);
  const [done, setDone] = useState(false);
  const last = info?.last_backup_ms ? new Date(info.last_backup_ms).toLocaleString(language) : t("backup.never");

  return (
    <>
      <h2>{t("backup.title")}</h2>
      <p className="hint">{t("backup.hint")}</p>
      <p>{rich(t("backup.last"), { date: <strong>{last}</strong> })}</p>
      <button
        type="button"
        disabled={backUp.busy}
        onClick={async () => setDone((await backUp.run()) === true)}
      >
        {t("backup.backUpNow")}
      </button>
      {done && <p role="status">{t("backup.saved")}</p>}
      <ErrorMessage error={backUp.error} />
    </>
  );
}
