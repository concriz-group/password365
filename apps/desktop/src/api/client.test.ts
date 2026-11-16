import { DEFAULT_SETTINGS } from "../settings";
import { ApiError, createClient, toApiError } from "./client";

describe("toApiError", () => {
  it("keeps ApiError instances", () => {
    const e = new ApiError("locked", "the vault is locked");
    expect(toApiError(e)).toBe(e);
  });

  it("maps backend { code, message } objects", () => {
    const e = toApiError({ code: "wrong_credentials", message: "nope" });
    expect(e).toBeInstanceOf(ApiError);
    expect(e.code).toBe("wrong_credentials");
    expect(e.message).toBe("nope");
  });

  it("wraps anything else as internal", () => {
    const e = toApiError("boom");
    expect(e.code).toBe("internal");
    expect(e.message).toBe("boom");
  });
});

describe("createClient", () => {
  const invoke = vi.fn();
  const client = createClient(invoke);

  beforeEach(() => invoke.mockReset());

  it("passes command names and camelCase args to invoke", async () => {
    invoke.mockResolvedValue(undefined);
    await client.unlock("pw");
    expect(invoke).toHaveBeenCalledWith("unlock", { password: "pw", secretKey: null });
    await client.unlock("pw", "P365-ABC");
    expect(invoke).toHaveBeenLastCalledWith("unlock", { password: "pw", secretKey: "P365-ABC" });
  });

  it("returns the backend result", async () => {
    invoke.mockResolvedValue({ initialized: true, unlocked: false });
    await expect(client.status()).resolves.toEqual({ initialized: true, unlocked: false });
  });

  it("rejects with ApiError", async () => {
    // A plain function: vi.fn() tracks returned promises and reports this rejection as unhandled.
    const failing = createClient(() => Promise.reject({ code: "locked", message: "the vault is locked" }));
    const error = await failing.vaults().catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).code).toBe("locked");
  });

  it("covers every command", async () => {
    invoke.mockResolvedValue(null);
    const calls: [() => Promise<unknown>, string, Record<string, unknown> | undefined][] = [
      [() => client.createAccount("a", "b"), "create_account", { password: "a", confirmation: "b" }],
      [() => client.lock(), "lock", undefined],
      [
        () => client.changeMasterPassword("c", "n", "n"),
        "change_master_password",
        { current: "c", newPassword: "n", confirmation: "n" },
      ],
      [() => client.createVault("W"), "create_vault", { name: "W" }],
      [() => client.renameVault("1", "X"), "rename_vault", { id: "1", name: "X" }],
      [() => client.deleteVault("1"), "delete_vault", { id: "1" }],
      [() => client.items("q"), "items", { query: "q", vaultId: null }],
      [() => client.items("q", "v"), "items", { query: "q", vaultId: "v" }],
      [() => client.item("1"), "item", { id: "1" }],
      [() => client.reveal("1", "password"), "reveal", { id: "1", field: "password" }],
      [() => client.createItem({ title: "t" }), "create_item", { input: { title: "t" } }],
      [() => client.updateItem("1", 2, { title: "t" }), "update_item", { id: "1", version: 2, input: { title: "t" } }],
      [() => client.deleteItem("1"), "delete_item", { id: "1" }],
      [() => client.generate({ type: "pin", digits: 4 }), "generate", { request: { type: "pin", digits: 4 } }],
      [() => client.importCsv("/f", true), "import_csv", { path: "/f", update: true, vaultId: null }],
      [() => client.importCsv("/f", false, "v"), "import_csv", { path: "/f", update: false, vaultId: "v" }],
      [() => client.exportCsv("/o"), "export_csv", { path: "/o", vaultId: null }],
      [() => client.exportCsv("/o", "v"), "export_csv", { path: "/o", vaultId: "v" }],
      [() => client.settings(), "settings", undefined],
      [() => client.printPage(), "print_page", undefined],
      [
        () => client.updateSettings({ ...DEFAULT_SETTINGS, auto_lock_minutes: 5, language: "es" }),
        "update_settings",
        { settings: { ...DEFAULT_SETTINGS, auto_lock_minutes: 5, language: "es" } },
      ],
    ];
    for (const [call, command, args] of calls) {
      await call();
      expect(invoke).toHaveBeenLastCalledWith(command, args);
    }
  });
});
