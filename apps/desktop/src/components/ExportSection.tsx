import { useState } from "react";

import type { Vault } from "../api/types";
import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";
import { rich } from "../i18n/richText";
import type { FilePicker } from "../lib/files";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  vaults: Vault[];
  files: FilePicker;
}

/**
 * Settings › Export: a plaintext copy for another app, apart from importing
 * (like 1Password, where export sits with the account, not with "Migrate").
 */
export function ExportSection({ vaults, files }: Props) {
  const client = useClient();
  const t = useT();
  const [exportVault, setExportVault] = useState("");
  const [acknowledged, setAcknowledged] = useState(false);
  const [exported, setExported] = useState<number | null>(null);
  const [exportedCxf, setExportedCxf] = useState<number | null>(null);
  const doExport = useAction(async () => {
    const path = await files.saveCsv("password365-export.csv");
    if (!path) return;
    setExported(await client.exportCsv(path, exportVault || undefined));
  });
  const doExportCxf = useAction(async () => {
    const path = await files.saveFile("password365-export.cxf.json");
    if (!path) return;
    setExportedCxf(await client.exportCxf(path, exportVault || undefined));
  });

  return (
    <section aria-label={t("transfer.exportTitle")}>
      <h2>{t("transfer.exportTitle")}</h2>
      <p className="hint warn">{rich(t("transfer.exportWarning"), { plain: <strong>{t("transfer.plainText")}</strong> })}</p>
      <div className="field">
        <label htmlFor="export-vault">{t("transfer.vault")}</label>
        <select id="export-vault" value={exportVault} onChange={(e) => setExportVault(e.target.value)}>
          <option value="">{t("transfer.allVaults")}</option>
          {vaults.map((v) => (
            <option key={v.id} value={v.id}>
              {v.name}
            </option>
          ))}
        </select>
      </div>
      <label className="checkbox">
        <input type="checkbox" checked={acknowledged} onChange={(e) => setAcknowledged(e.target.checked)} />
        {t("transfer.acknowledge")}
      </label>
      <button type="button" onClick={() => void doExport.run()} disabled={!acknowledged || doExport.busy}>
        {t("transfer.exportCsv")}
      </button>
      <button type="button" className="ghost" onClick={() => void doExportCxf.run()} disabled={!acknowledged || doExportCxf.busy}>
        {t("transfer.exportCxf")}
      </button>
      <p className="hint">{t("transfer.cxfHint")}</p>
      <ErrorMessage error={doExport.error ?? doExportCxf.error} />
      {exported !== null && <p role="status">{t("transfer.exported", { count: exported })}</p>}
      {exportedCxf !== null && <p role="status">{t("transfer.exportedCxf", { count: exportedCxf })}</p>}
    </section>
  );
}
