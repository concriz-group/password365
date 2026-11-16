import type { FilePicker } from "../lib/files";

/** File pickers that "choose" fixed paths; override what a test needs. */
export function fakeFiles(overrides: Partial<FilePicker> = {}): FilePicker {
  return {
    openCsv: vi.fn().mockResolvedValue("/tmp/in.csv"),
    openImport: vi.fn().mockResolvedValue("/tmp/export.1pux"),
    openAnyFile: vi.fn().mockResolvedValue("/tmp/passport.pdf"),
    saveFile: vi.fn().mockResolvedValue("/tmp/copy.pdf"),
    saveCsv: vi.fn().mockResolvedValue("/tmp/out.csv"),
    openBackup: vi.fn().mockResolvedValue("/tmp/old.p365backup"),
    saveBackup: vi.fn().mockResolvedValue("/tmp/new.p365backup"),
    openSyncFile: vi.fn().mockResolvedValue("/tmp/in.p365sync"),
    saveSyncFile: vi.fn().mockResolvedValue("/tmp/out.p365sync"),
    ...overrides,
  };
}
