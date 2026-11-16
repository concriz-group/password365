import { open, save } from "@tauri-apps/plugin-dialog";

import { nativeFilePicker } from "./files";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

describe("nativeFilePicker", () => {
  it("opens a single CSV file", async () => {
    vi.mocked(open).mockResolvedValue("/a.csv");
    await expect(nativeFilePicker.openCsv()).resolves.toBe("/a.csv");
    expect(open).toHaveBeenCalledWith(expect.objectContaining({ multiple: false, directory: false }));
  });

  it("returns null when nothing was picked", async () => {
    vi.mocked(open).mockResolvedValue(null);
    await expect(nativeFilePicker.openCsv()).resolves.toBeNull();
  });

  it("asks for a save location with a default name", async () => {
    vi.mocked(save).mockResolvedValue("/b.csv");
    await expect(nativeFilePicker.saveCsv("x.csv")).resolves.toBe("/b.csv");
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ defaultPath: "x.csv" }));
  });

  it("opens and saves Password365 backups with their own filter", async () => {
    vi.mocked(open).mockResolvedValue("/old.qlbackup");
    await expect(nativeFilePicker.openBackup()).resolves.toBe("/old.qlbackup");
    expect(open).toHaveBeenLastCalledWith(
      expect.objectContaining({ filters: [{ name: "Password365 backup", extensions: ["qlbackup"] }] }),
    );
    vi.mocked(save).mockResolvedValue("/new.qlbackup");
    await expect(nativeFilePicker.saveBackup("q.qlbackup")).resolves.toBe("/new.qlbackup");
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ defaultPath: "q.qlbackup" }));
  });

  it("opens and saves sync files with their own filter", async () => {
    vi.mocked(open).mockResolvedValue("/in.qlsync");
    await expect(nativeFilePicker.openSyncFile()).resolves.toBe("/in.qlsync");
    expect(open).toHaveBeenLastCalledWith(
      expect.objectContaining({ filters: [{ name: "Password365 sync file", extensions: ["qlsync"] }] }),
    );
    vi.mocked(save).mockResolvedValue("/out.qlsync");
    await expect(nativeFilePicker.saveSyncFile("x.qlsync")).resolves.toBe("/out.qlsync");
  });
});
