/** Default backup file name, e.g. `password365-backup-2026-11-09.qlbackup` (local date). */
export function backupFileName(now = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `password365-backup-${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}.qlbackup`;
}
