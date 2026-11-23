// Typed wrapper around the Tauri IPC commands in `src-tauri/src/commands.rs`.

import type {
  AppStatus,
  AttachmentInfo,
  BackupInfo,
  Breached,
  Category,
  DeletedItem,
  Device,
  ErrorCode,
  GenerateRequest,
  Generated,
  ImportSummary,
  Invite,
  Item,
  ItemInput,
  ItemSummary,
  PasswordStrength,
  PasswordUpdate,
  Profile,
  ProfileKind,
  SecretField,
  Settings,
  SshKeys,
  SyncResult,
  Tag,
  ThisDevice,
  TotpCode,
  Vault,
  Watchtower,
  UpdateInfo,
  HelpTopic,
} from "./types";

/** Error returned by the backend: `{ code, message }`. */
export class ApiError extends Error {
  readonly code: ErrorCode;

  constructor(code: ErrorCode, message: string) {
    super(message);
    this.name = "ApiError";
    this.code = code;
  }
}

export type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

function isBackendError(value: unknown): value is { code: ErrorCode; message: string } {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

/** Converts whatever the backend rejected with into an `ApiError`. */
export function toApiError(reason: unknown): ApiError {
  if (reason instanceof ApiError) return reason;
  if (isBackendError(reason)) return new ApiError(reason.code, reason.message);
  return new ApiError("internal", String(reason));
}

export function createClient(invoke: Invoke) {
  async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    try {
      return (await invoke(command, args)) as T;
    } catch (reason) {
      throw toApiError(reason);
    }
  }

  return {
    status: () => call<AppStatus>("status"),
    createAccount: (password: string, confirmation: string) =>
      call<string>("create_account", { password, confirmation }),
    unlock: (password: string, secretKey?: string) =>
      call<void>("unlock", { password, secretKey: secretKey || null }),
    lock: () => call<void>("lock"),
    userActive: () => call<void>("user_active"),
    passwordStrength: (password: string) => call<PasswordStrength>("password_strength", { password }),
    changeMasterPassword: (current: string, newPassword: string, confirmation: string) =>
      call<void>("change_master_password", { current, newPassword, confirmation }),

    vaults: () => call<Vault[]>("vaults"),
    createVault: (name: string) => call<Vault>("create_vault", { name }),
    renameVault: (id: string, name: string) => call<void>("rename_vault", { id, name }),
    deleteVault: (id: string) => call<void>("delete_vault", { id }),

    items: (query: string, vaultId?: string) =>
      call<ItemSummary[]>("items", { query, vaultId: vaultId ?? null }),
    item: (id: string) => call<Item>("item", { id }),
    reveal: (id: string, field: SecretField) => call<string | null>("reveal", { id, field }),
    /** The item's one-time password, or that of its one-time password field `label`. */
    totpCode: (id: string, label?: string) => call<TotpCode | null>("totp_code", { id, label: label ?? null }),
    createItem: (input: ItemInput) => call<Item>("create_item", { input }),
    updateItem: (id: string, version: number, input: ItemInput) =>
      call<Item>("update_item", { id, version, input }),
    deleteItem: (id: string) => call<void>("delete_item", { id }),
    markReviewed: (id: string) => call<void>("mark_reviewed", { id }),
    tags: () => call<Tag[]>("tags"),
    setFavorite: (id: string, favorite: boolean) => call<void>("set_favorite", { id, favorite }),
    profile: () => call<Profile>("profile"),
    setProfile: (kind: ProfileKind, id: string | null) => call<void>("set_profile", { kind, id }),
    autoType: (id: string) => call<void>("auto_type", { id }),
    /** Hides the Quick Access window (no-op elsewhere). */
    hideQuickAccess: () => call<void>("hide_quick_access"),
    /** Brings the main window forward, on item `id` (or just the app, to unlock). */
    openInApp: (id: string | null) => call<void>("open_in_app", { id }),
    watchtower: () => call<Watchtower>("watchtower"),
    categories: () => call<Category[]>("categories"),
    revealField: (id: string, label: string) => call<string | null>("reveal_field", { id, label }),
    archivedItems: (query: string) => call<ItemSummary[]>("archived_items", { query }),
    setArchived: (id: string, archived: boolean) => call<void>("set_archived", { id, archived }),
    /** QR code (SVG) joining a Wireless Router item's network, or null. */
    /** Every item's own icon (data URL) by item id. */
    importKdbx: (path: string, password: string, vaultId?: string, keyFile?: string) =>
      call<ImportSummary>("import_kdbx", { path, password, keyFile: keyFile ?? null, vaultId: vaultId ?? null }),
    importBitwarden: (path: string, vaultId?: string) => call<ImportSummary>("import_bitwarden", { path, vaultId: vaultId ?? null }),
    import1pux: (path: string, vaultId?: string) => call<ImportSummary>("import_1pux", { path, vaultId: vaultId ?? null }),
    /** Copies a secret, hidden from clipboard history; cleared after `clearAfterMs` (0: kept). */
    copySecret: (text: string, clearAfterMs: number) => call<void>("copy_secret", { text, clearAfterMs }),
    attachFile: (id: string, path: string) => call<AttachmentInfo>("attach_file", { id, path }),
    saveAttachment: (id: string, attachmentId: string, path: string) => call<void>("save_attachment", { id, attachmentId, path }),
    openAttachment: (id: string, attachmentId: string) => call<void>("open_attachment", { id, attachmentId }),
    importCxf: (path: string, vaultId?: string) => call<ImportSummary>("import_cxf", { path, vaultId: vaultId ?? null }),
    exportCxf: (path: string, vaultId: string | undefined, password: string) =>
      call<number>("export_cxf", { path, vaultId: vaultId ?? null, password }),
    removePasskey: (id: string) => call<void>("remove_passkey", { id }),
    removeAttachment: (id: string, attachmentId: string) => call<void>("remove_attachment", { id, attachmentId }),
    /** Opens a help page (bug, idea, translate, security, email): fixed addresses in the backend. */
    openHelp: (topic: HelpTopic) => call<void>("open_help", { topic }),
    /** Asks for a newer version (null: up to date). */
    updateCheck: () => call<UpdateInfo | null>("update_check"),
    /** Installs the version found by the last check and restarts the app. */
    updateInstall: () => call<void>("update_install"),
    breachedPasswords: () => call<Breached[]>("breached_passwords"),
    sshKeys: () => call<SshKeys>("ssh_keys"),
    /** The user's answer to an SSH key approval prompt. */
    sshAnswer: (id: number, allow: boolean) => call<void>("ssh_answer", { id, allow }),
    itemIcons: () => call<Record<string, string>>("item_icons"),
    wifiQr: (id: string) => call<string | null>("wifi_qr", { id }),
    moveItem: (id: string, vaultId: string) => call<Item>("move_item", { id, vaultId }),
    duplicateItem: (id: string, title: string) => call<Item>("duplicate_item", { id, title }),
    deletedItems: () => call<DeletedItem[]>("deleted_items"),
    restoreItem: (id: string) => call<void>("restore_item", { id }),
    deletePermanently: (id: string) => call<void>("delete_permanently", { id }),

    generate: (request: GenerateRequest) => call<Generated>("generate", { request }),
    importCsv: (path: string, update: boolean, vaultId?: string) =>
      call<ImportSummary>("import_csv", { path, update, vaultId: vaultId ?? null }),
    exportCsv: (path: string, vaultId: string | undefined, password: string) =>
      call<number>("export_csv", { path, vaultId: vaultId ?? null, password }),

    exportBackup: (path: string) => call<BackupInfo>("export_backup", { path }),
    backupInfo: () => call<BackupInfo>("backup_info"),
    restoreBackup: (path: string, password: string, secretKey: string) =>
      call<void>("restore_backup", { path, password, secretKey }),

    thisDevice: () => call<ThisDevice>("this_device"),
    renameThisDevice: (name: string) => call<ThisDevice>("rename_this_device", { name }),
    devices: () => call<Device[]>("devices"),
    removeDevice: (id: string) => call<void>("remove_device", { id }),
    syncDevice: (id: string) => call<SyncResult>("sync_device", { id }),
    exportSyncFile: (id: string, path: string) => call<number>("export_sync_file", { id, path }),
    importSyncFile: (path: string) => call<SyncResult>("import_sync_file", { path }),
    inviteDevice: () => call<Invite>("invite_device"),
    waitForNewDevice: () => call<void>("wait_for_new_device"),
    approveNewDevice: (code: string, vaultIds: string[]) => call<Device>("approve_new_device", { code, vaultIds }),
    cancelPairing: () => call<void>("cancel_pairing"),
    joinWithCode: (code: string) => call<string>("join_with_code", { code }),
    /** New device: shows a QR code (and code) for the device that has the vault to scan. */
    joinShowCode: () => call<Invite>("join_show_code"),
    /** New device: waits until that device scans it; returns the six digits to show. */
    joinAcceptScan: () => call<string>("join_accept_scan"),
    /** Device with the vault: connects to the code a new device shows (pasted). */
    connectNewDevice: (code: string) => call<void>("connect_new_device", { code }),
    joinWait: () => call<void>("join_wait"),
    joinComplete: (password: string, deviceName: string) =>
      call<void>("join_complete", { password, deviceName }),

    settings: () => call<Settings>("settings"),
    updateSettings: (settings: Settings) => call<void>("update_settings", { settings }),
    printPage: () => call<void>("print_page"),
    /** Erases this computer's vault for good (works while locked). */
    eraseVault: () => call<void>("erase_vault"),
    revealSecretKey: (password: string) => call<string>("reveal_secret_key", { password }),
    /** Keeps the Emergency Kit PDF in the vault (once) and returns the item's id. */
    saveEmergencyKit: (password: string, text: KitText) => call<string>("save_emergency_kit", { password, text }),
    pendingPasswordChange: () => call<PasswordUpdate | null>("pending_password_change"),
    adoptMasterPassword: (password: string) => call<void>("adopt_master_password", { password }),
  };
}

export type Client = ReturnType<typeof createClient>;

/** The Emergency Kit sheet's words, already translated. */
export interface KitText {
  title: string;
  created: string;
  keep: string;
  secret_key_label: string;
  master_password_label: string;
  write_here: string;
  how_to: string;
  steps: string[];
}
