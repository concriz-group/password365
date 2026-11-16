import { open, save } from "@tauri-apps/plugin-dialog";

/** Native file pickers (injectable for tests). Each returns a path or `null` if cancelled. */
export interface FilePicker {
  openCsv(): Promise<string | null>;
  /** A CSV or a 1Password export (.1pux). */
  openImport(): Promise<string | null>;
  /** Any file (to attach to an item). */
  openAnyFile(): Promise<string | null>;
  /** Where to save a copy of an attachment. */
  saveFile(defaultName: string): Promise<string | null>;
  saveCsv(defaultName: string): Promise<string | null>;
  openBackup(): Promise<string | null>;
  saveBackup(defaultName: string): Promise<string | null>;
  openSyncFile(): Promise<string | null>;
  saveSyncFile(defaultName: string): Promise<string | null>;
}

const CSV = [{ name: "CSV", extensions: ["csv"] }];
const IMPORT = [{ name: "CSV, 1Password, Bitwarden or KeePass export", extensions: ["csv", "1pux", "json", "kdbx"] }];
const BACKUP = [{ name: "Password365 backup", extensions: ["p365backup"] }];
const SYNC_FILE = [{ name: "Password365 sync file", extensions: ["p365sync"] }];
// Files made before the rename (.qlbackup, .qlsync) still open; new ones are saved with the new names.
const OPEN_BACKUP = [{ name: "Password365 backup", extensions: ["p365backup", "qlbackup"] }];
const OPEN_SYNC_FILE = [{ name: "Password365 sync file", extensions: ["p365sync", "qlsync"] }];

async function openOne(filters: typeof CSV): Promise<string | null> {
  const picked = await open({ multiple: false, directory: false, filters });
  return typeof picked === "string" ? picked : null;
}

export const nativeFilePicker: FilePicker = {
  openCsv: () => openOne(CSV),
  openImport: () => openOne(IMPORT),
  openAnyFile: async () => {
    const picked = await open({ multiple: false, directory: false });
    return typeof picked === "string" ? picked : null;
  },
  saveFile: (defaultName) => save({ defaultPath: defaultName }),
  saveCsv: (defaultName) => save({ defaultPath: defaultName, filters: CSV }),
  openBackup: () => openOne(OPEN_BACKUP),
  saveBackup: (defaultName) => save({ defaultPath: defaultName, filters: BACKUP }),
  openSyncFile: () => openOne(OPEN_SYNC_FILE),
  saveSyncFile: (defaultName) => save({ defaultPath: defaultName, filters: SYNC_FILE }),
};
