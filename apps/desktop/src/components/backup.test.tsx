import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { backupFileName } from "../lib/backupName";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { fakeFiles } from "../test/fakeFiles";
import { BackupReminder } from "./BackupReminder";
import { BackupSection } from "./BackupSection";

const DAY = 24 * 60 * 60 * 1000;

describe("backups", () => {
  it("names backups by date", () => {
    expect(backupFileName(new Date(2026, 0, 5))).toBe("password365-backup-2026-01-05.qlbackup");
  });

  it("settings show the last backup and export a new one", async () => {
    const user = userEvent.setup();
    const exportBackup = vi.fn().mockResolvedValue({ last_backup_ms: Date.now(), due: false });
    const client = fakeClient({ backupInfo: vi.fn().mockResolvedValue({ last_backup_ms: null, due: true }), exportBackup });
    const files = fakeFiles();
    renderWithClient(<BackupSection files={files} />, client);
    expect(await screen.findByText("never")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back up now…" }));
    expect(files.saveBackup).toHaveBeenCalledWith(expect.stringMatching(/^password365-backup-.*\.qlbackup$/));
    expect(exportBackup).toHaveBeenCalledWith("/tmp/new.qlbackup");
    expect(await screen.findByRole("status")).toHaveTextContent("Backup saved.");
    expect(screen.queryByText("never")).toBeNull();
  });

  it("does nothing when the save dialog is cancelled, and shows export errors", async () => {
    const user = userEvent.setup();
    const exportBackup = vi.fn().mockRejectedValue({ code: "io", message: "disk full" });
    const client = fakeClient({ backupInfo: vi.fn().mockResolvedValue({ last_backup_ms: null, due: true }), exportBackup });
    const files = fakeFiles({ saveBackup: vi.fn().mockResolvedValueOnce(null).mockResolvedValue("/tmp/x") });
    renderWithClient(<BackupSection files={files} />, client);
    await user.click(screen.getByRole("button", { name: "Back up now…" }));
    expect(exportBackup).not.toHaveBeenCalled();
    expect(screen.queryByRole("status")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Back up now…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
  });

  it("reminds only when there are items and a backup is due", async () => {
    const user = userEvent.setup();
    const old = { last_backup_ms: Date.now() - 40 * DAY, due: true };
    const exportBackup = vi.fn().mockResolvedValue({ last_backup_ms: Date.now(), due: false });
    const client = fakeClient({ backupInfo: vi.fn().mockResolvedValue(old), exportBackup });

    const empty = renderWithClient(<BackupReminder files={fakeFiles()} hasItems={false} />, client);
    await waitFor(() => expect(client.backupInfo).toHaveBeenCalled());
    expect(screen.queryByRole("note")).toBeNull();
    empty.unmount();

    renderWithClient(<BackupReminder files={fakeFiles()} hasItems />, client);
    expect(await screen.findByRole("note")).toHaveTextContent("No backup in the last 30 days.");
    await user.click(screen.getByRole("button", { name: "Back up now…" }));
    await waitFor(() => expect(screen.queryByRole("note")).toBeNull());
  });

  it("says when there was never a backup, and stays quiet if the status can't be read", async () => {
    const never = fakeClient({ backupInfo: vi.fn().mockResolvedValue({ last_backup_ms: null, due: true }) });
    const first = renderWithClient(<BackupReminder files={fakeFiles()} hasItems />, never);
    expect(await screen.findByRole("note")).toHaveTextContent("You have no backup yet.");
    first.unmount();
    const broken = fakeClient({ backupInfo: vi.fn().mockRejectedValue({ code: "internal", message: "x" }) });
    renderWithClient(<BackupReminder files={fakeFiles()} hasItems />, broken);
    await waitFor(() => expect(broken.backupInfo).toHaveBeenCalled());
    expect(screen.queryByRole("note")).toBeNull();
  });
});
