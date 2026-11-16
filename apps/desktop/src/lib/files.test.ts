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
    vi.mocked(open).mockResolvedValue("/old.p365backup");
    await expect(nativeFilePicker.openBackup()).resolves.toBe("/old.p365backup");
    expect(open).toHaveBeenLastCalledWith(
      expect.objectContaining({ filters: [{ name: "Password365 backup", extensions: ["p365backup", "qlbackup"] }] }),
    );
    vi.mocked(save).mockResolvedValue("/new.p365backup");
    await expect(nativeFilePicker.saveBackup("q.p365backup")).resolves.toBe("/new.p365backup");
    expect(save).toHaveBeenLastCalledWith(
      expect.objectContaining({ defaultPath: "q.p365backup", filters: [{ name: "Password365 backup", extensions: ["p365backup"] }] }),
    );
  });

  it("opens and saves sync files with their own filter", async () => {
    vi.mocked(open).mockResolvedValue("/in.p365sync");
    await expect(nativeFilePicker.openSyncFile()).resolves.toBe("/in.p365sync");
    expect(open).toHaveBeenLastCalledWith(
      expect.objectContaining({ filters: [{ name: "Password365 sync file", extensions: ["p365sync", "qlsync"] }] }),
    );
    vi.mocked(save).mockResolvedValue("/out.p365sync");
    await expect(nativeFilePicker.saveSyncFile("x.p365sync")).resolves.toBe("/out.p365sync");
  });
});
