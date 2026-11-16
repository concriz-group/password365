import { useState } from "react";

import type { ImportSummary, Vault } from "../api/types";
import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";
import type { FilePicker } from "../lib/files";
import type { ImportSource } from "../lib/importSources";
import { ErrorMessage } from "./ErrorMessage";
import { MigratePicker, useSourceName } from "./MigratePicker";
import { PasswordInput } from "./PasswordInput";

/** Sources whose own full export (not only CSV) can be read. */
const FULL_EXPORTS = new Set(["onepassword", "bitwarden", "keepass", "keepassxc", "cxf"]);
/** Sources whose database is opened with its own password. */
const KEEPASS = new Set(["keepass", "keepassxc"]);

/** What the import did, as message keys (`transfer.<key>`) with their counts. */
export function summaryLines(s: ImportSummary): [string, number][] {
  const { imported, identical, changed, updated, duplicates_in_file: duplicates } = s;
  if (imported + changed + updated + duplicates === 0) {
    return identical > 0 ? [["nothingNew", identical]] : s.skipped.length > 0 ? [] : [["nothingToImport", 0]];
  }
  const lines: [string, number][] = [
    ["importedCount", imported],
    ["updatedCount", updated],
    ["changedCount", changed],
    ["identicalCount", identical],
    ["duplicatesCount", duplicates],
  ];
  return lines.filter(([key, count]) => count > 0 || key === "importedCount");
}

interface Props {
  vaults: Vault[];
  files: FilePicker;
  onImported: () => void;
  onClose: () => void;
}

/** "Migrate your data", like 1Password: import from a browser or another manager. */
export function Transfer({ vaults, files, onImported, onClose }: Props) {
  const client = useClient();
  const t = useT();
  const [importVault, setImportVault] = useState(vaults[0]?.id ?? "");
  const [update, setUpdate] = useState(false);
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [source, setSource] = useState<ImportSource | null>(null);
  const [databasePassword, setDatabasePassword] = useState("");
  const [keyFile, setKeyFile] = useState<string | null>(null);
  const nameOf = useSourceName();

  const doImport = useAction(async () => {
    const path = await (source && FULL_EXPORTS.has(source.id) ? files.openImport() : files.openCsv());
    if (!path) return;
    const vault = importVault || undefined;
    const lower = path.toLowerCase();
    setSummary(
      source?.id === "cxf"
        ? await client.importCxf(path, vault)
        : lower.endsWith(".1pux")
        ? await client.import1pux(path, vault)
        : lower.endsWith(".json")
          ? await client.importBitwarden(path, vault)
          : lower.endsWith(".kdbx")
            ? await client.importKdbx(path, databasePassword, vault, keyFile ?? undefined)
            : await client.importCsv(path, update, vault),
    );
    setDatabasePassword("");
    onImported();
  });

  return (
    <section className="transfer" aria-label={t("transfer.region")}>
      <header className="toolbar-row">
        <h1>{t("transfer.title")}</h1>
        <button type="button" className="ghost" onClick={onClose}>
          {t("common.close")}
        </button>
      </header>

      <h2>{t("migrate.title")}</h2>
      {source === null ? (
        <MigratePicker onPick={setSource} />
      ) : (
        <>
          <div className="import-source">
            <h3>{t("migrate.from", { source: nameOf(source) })}</h3>
            <button type="button" className="ghost" onClick={() => setSource(null)}>
              {t("migrate.back")}
            </button>
          </div>
          <p className="hint">{t(`migrate.steps_${source.id}`)}</p>
          <p className="hint">{t("migrate.keeps")}</p>
          <div className="field">
            <label htmlFor="import-vault">{t("transfer.intoVault")}</label>
            <select id="import-vault" value={importVault} onChange={(e) => setImportVault(e.target.value)}>
              {vaults.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name}
                </option>
              ))}
            </select>
          </div>
          {KEEPASS.has(source.id) && (
            <>
              <PasswordInput label={t("migrate.keepassPassword")} value={databasePassword} onChange={setDatabasePassword} autoComplete="off" />
              <div className="field-row">
                <button type="button" className="ghost" onClick={() => void files.openAnyFile().then((p) => p && setKeyFile(p))}>
                  {t("migrate.keyFile")}
                </button>
                {keyFile && (
                  <>
                    <span className="muted">{keyFile.split(/[\\/]/).pop()}</span>
                    <button type="button" className="link" onClick={() => setKeyFile(null)}>
                      {t("common.remove")}
                    </button>
                  </>
                )}
              </div>
            </>
          )}
          {/* Full exports never overwrite: only a CSV can update passwords. */}
          {!FULL_EXPORTS.has(source.id) && (
            <label className="checkbox">
              <input type="checkbox" checked={update} onChange={(e) => setUpdate(e.target.checked)} />
              {t("transfer.updateExisting")}
            </label>
          )}
          <button type="button" onClick={() => void doImport.run()} disabled={doImport.busy}>
            {FULL_EXPORTS.has(source.id) ? t("migrate.chooseExport") : t("transfer.chooseCsv")}
          </button>
        </>
      )}
      <ErrorMessage error={doImport.error} />
      {summary && (
        <div role="status" className="summary">
          {summaryLines(summary).map(([key, count]) => (
            <p key={key}>{t(`transfer.${key}`, { count })}</p>
          ))}
          {summary.skipped.map(([line, reason]) => (
            <p key={line} className="hint">
              {t("transfer.skipped", { line, reason })}
            </p>
          ))}
          <p className="hint warn">{t("transfer.deleteFile")}</p>
        </div>
      )}

    </section>
  );
}
