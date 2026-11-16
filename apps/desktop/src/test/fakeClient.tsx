import { render } from "@testing-library/react";
import type { ReactElement } from "react";

import type { Client } from "../api/client";
import { ClientProvider } from "../api/context";
import { roughStrength } from "../dev/roughStrength";

/** A client whose every method is a `vi.fn()`; override what a test needs. */
export function fakeClient(overrides: Partial<Record<keyof Client, unknown>> = {}): Client {
  const methods = [
    "status", "createAccount", "unlock", "lock", "changeMasterPassword",
    "vaults", "createVault", "renameVault", "deleteVault",
    "items", "item", "reveal", "totpCode", "createItem", "updateItem", "deleteItem", "markReviewed", "tags", "setFavorite", "archivedItems", "setArchived", "moveItem", "wifiQr", "itemIcons", "sshKeys", "breachedPasswords", "import1pux", "importBitwarden", "importKdbx", "copySecret", "attachFile", "saveAttachment", "openAttachment", "removeAttachment", "removePasskey", "exportCxf", "importCxf", "sshAnswer", "duplicateItem", "hideQuickAccess", "openInApp", "categories", "revealField", "watchtower", "autoType", "profile", "setProfile",
    "deletedItems", "restoreItem", "deletePermanently",
    "generate", "importCsv", "exportCsv", "settings", "updateSettings", "printPage", "revealSecretKey", "pendingPasswordChange", "adoptMasterPassword",
    "exportBackup", "backupInfo", "restoreBackup",
    "thisDevice", "renameThisDevice", "devices", "removeDevice", "syncDevice",
    "exportSyncFile", "importSyncFile", "updateCheck", "updateInstall", "openHelp",
    "inviteDevice", "waitForNewDevice", "approveNewDevice", "cancelPairing",
    "joinWithCode", "joinWait", "joinComplete",
  ] as const;
  const lists = new Set(["vaults", "items", "deletedItems", "devices", "tags", "archivedItems", "categories"]);
  const client = Object.fromEntries(
    methods.map((m) => [m, vi.fn().mockResolvedValue(lists.has(m) ? [] : undefined)]),
  );
  client.passwordStrength = vi.fn(async (password: string) => roughStrength(password));
  return { ...client, ...overrides } as unknown as Client;
}

/** Renders `ui` with the given client in context. */
export function renderWithClient(ui: ReactElement, client: Client) {
  return render(<ClientProvider client={client}>{ui}</ClientProvider>);
}
