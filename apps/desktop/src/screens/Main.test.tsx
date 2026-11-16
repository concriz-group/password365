import { act, screen, waitFor, within } from "@testing-library/react";

import type { AutoTypeChoice } from "../api/types";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { item, personal, summary, work, categories } from "../test/fixtures";
import { Main } from "./Main";
import { ClientProvider } from "../api/context";
import { render } from "@testing-library/react";
import { DEFAULT_SETTINGS, SettingsProvider } from "../settings";
import type { Settings } from "../api/types";

/** Main with this device's settings. */
function withSettings(change: Partial<Settings>, overrides = {}) {
  const client = fakeClient({
    vaults: vi.fn().mockResolvedValue([personal, work]),
    categories: vi.fn().mockResolvedValue(categories),
    items: vi.fn().mockResolvedValue([summary, { ...summary, id: "n1", kind: "note", title: "Note", favorite: true }]),
    item: vi.fn().mockResolvedValue(item),
    settings: vi.fn().mockResolvedValue({ ...DEFAULT_SETTINGS, ...change }),
    watchtower: vi.fn().mockResolvedValue({ score: null, rating: null, strength: { strong: 0, good: 0, weak: 0 }, weak: [], reused: [], unsecured: [], expiring: [] }),
    ...overrides,
  });
  const ui = (
    <ClientProvider client={client}>
      <SettingsProvider>
        <Main onLocked={vi.fn()} copy={vi.fn()} />
      </SettingsProvider>
    </ClientProvider>
  );
  render(ui);
  return { client, user: userEvent.setup() };
}

function setup(overrides = {}) {
  const client = fakeClient({
    vaults: vi.fn().mockResolvedValue([personal, work]),
    categories: vi.fn().mockResolvedValue(categories),
    items: vi.fn().mockResolvedValue([summary]),
    item: vi.fn().mockResolvedValue(item),
    createItem: vi.fn().mockResolvedValue(item),
    createVault: vi.fn().mockResolvedValue(work),
    generate: vi.fn().mockResolvedValue({ value: "x", bits: 1, strength: "weak" }),
    ...overrides,
  });
  const onLocked = vi.fn();
  const copy = vi.fn().mockResolvedValue(undefined);
  renderWithClient(<Main onLocked={onLocked} copy={copy} />, client);
  return { client, onLocked, copy, user: userEvent.setup() };
}

describe("Main", () => {
  it("shows vaults and items, opens an item", async () => {
    const { user } = setup();
    expect(await screen.findByText("Select an item or create a new one.")).toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: /GitHub/ }));
    expect(await screen.findByRole("heading", { name: "GitHub" })).toBeInTheDocument();
    expect(screen.getByText("Login · Personal")).toBeInTheDocument();
  });

  it("creates an item and shows it", async () => {
    const { client, user } = setup();
    await user.click(screen.getByRole("button", { name: "+ New item" }));
    await user.click(await screen.findByRole("button", { name: /^Login$/ }));
    await user.type(screen.getByLabelText("Title"), "GitHub");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.createItem).toHaveBeenCalled();
    expect(await screen.findByRole("heading", { name: "GitHub" })).toBeInTheDocument();
  });

  it("edit then cancel returns to the item; cancel on new returns to empty", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: /GitHub/ }));
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    expect(screen.getByRole("form", { name: "Edit item" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByRole("heading", { name: "GitHub" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "+ New item" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByText("Select an item or create a new one.")).toBeInTheDocument();
  });

  it("deletes an item", async () => {
    const { client, user } = setup();
    await user.click(await screen.findByRole("button", { name: /GitHub/ }));
    await user.click(await screen.findByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Move to Recently Deleted" }));
    expect(client.deleteItem).toHaveBeenCalledWith("i1");
    expect(await screen.findByText("Select an item or create a new one.")).toBeInTheDocument();
  });

  it("opens Recently Deleted and goes back to the items", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "Recently Deleted" }));
    expect(await screen.findByRole("heading", { name: "Recently Deleted" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /All items/ }));
    expect(screen.getByText("Select an item or create a new one.")).toBeInTheDocument();
  });

  it("refreshes the vault counts after restoring an item", async () => {
    const deletedItems = vi
      .fn()
      .mockResolvedValueOnce([{ ...summary, deleted_at: 0, purge_at: Date.now() }])
      .mockResolvedValue([]);
    const { client, user } = setup({ deletedItems });
    await user.click(screen.getByRole("button", { name: "Recently Deleted" }));
    const loads = vi.mocked(client.vaults).mock.calls.length;
    await user.click(await screen.findByRole("button", { name: "Restore" }));
    await waitFor(() => expect(vi.mocked(client.vaults).mock.calls.length).toBeGreaterThan(loads));
  });

  it("opens the Devices screen", async () => {
    const { client, user } = setup({ thisDevice: vi.fn().mockResolvedValue({ id: "me", name: "Laptop" }) });
    await user.click(screen.getByRole("button", { name: "Devices" }));
    expect(await screen.findByRole("heading", { name: "Devices" })).toBeInTheDocument();
    expect(client.devices).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.getByText("Select an item or create a new one.")).toBeInTheDocument();
  });

  it("reloads after a background sync and stops listening when closed", async () => {
    let notify = () => {};
    const stop = vi.fn();
    const subscribeSync = vi.fn((cb: () => void) => {
      notify = cb;
      return stop;
    });
    const client = fakeClient({ vaults: vi.fn().mockResolvedValue([personal]), items: vi.fn().mockResolvedValue([]) });
    const view = renderWithClient(<Main onLocked={vi.fn()} subscribeSync={subscribeSync} />, client);
    await waitFor(() => expect(client.items).toHaveBeenCalled());
    const before = vi.mocked(client.items).mock.calls.length;
    notify();
    await waitFor(() => expect(vi.mocked(client.items).mock.calls.length).toBeGreaterThan(before));
    view.unmount();
    expect(stop).toHaveBeenCalled();
  });

  it("creates and selects a vault", async () => {
    const { client, user } = setup();
    await user.click(screen.getByRole("button", { name: "+ New vault" }));
    await user.type(screen.getByLabelText("Vault name"), "Work");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(client.createVault).toHaveBeenCalledWith("Work");
    await waitFor(() => expect(client.items).toHaveBeenLastCalledWith("", "v2"));
  });

  it("reports vault creation errors", async () => {
    const { user } = setup({ createVault: vi.fn().mockRejectedValue({ code: "validation", message: "bad name" }) });
    await user.click(screen.getByRole("button", { name: "+ New vault" }));
    await user.type(screen.getByLabelText("Vault name"), "x");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("bad name");
  });

  it("opens and closes the generator", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "Generator" }));
    expect(await screen.findByRole("region", { name: "Password generator" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.getByText("Select an item or create a new one.")).toBeInTheDocument();
  });

  it("opens import, and export inside settings", async () => {
    const { user } = setup({ settings: vi.fn().mockResolvedValue({ auto_lock_minutes: 10 }) });
    await user.click(screen.getByRole("button", { name: "Import" }));
    expect(screen.getByRole("region", { name: "Import" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Export CSV…" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("region", { name: "Settings" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Export" }));
    expect(screen.getByRole("button", { name: "Export CSV…" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.getByText("Select an item or create a new one.")).toBeInTheDocument();
  });

  it("locks", async () => {
    const { client, onLocked, user } = setup();
    await user.click(screen.getByRole("button", { name: "Lock" }));
    expect(client.lock).toHaveBeenCalled();
    expect(onLocked).toHaveBeenCalled();
  });

  it("opens Watchtower and an item from it", async () => {
    const { user } = setup({
      watchtower: vi.fn().mockResolvedValue({
        score: 500,
        rating: "fair",
        strength: { strong: 0, good: 0, weak: 1 },
        weak: [{ id: "i1", title: "GitHub", kind: "login" }],
        reused: [],
        unsecured: [],
        expiring: [],
      }),
    });
    await user.click(screen.getByRole("button", { name: "Watchtower" }));
    await user.click(await screen.findByRole("button", { name: "Show items" }));
    await user.click(within(screen.getByRole("list", { name: "Weak passwords" })).getByRole("button", { name: /GitHub/ }));
    expect(await screen.findByRole("heading", { name: "GitHub" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Watchtower" }));
    await user.click(screen.getByRole("button", { name: /All items/ }));
    expect(screen.queryByRole("region", { name: "Watchtower" })).not.toBeInTheDocument();
  });

  it("shows the auto-type choice the shortcut asks for", async () => {
    let fire: ((c: AutoTypeChoice) => void) | undefined;
    const client = fakeClient({ vaults: vi.fn().mockResolvedValue([personal]), categories: vi.fn().mockResolvedValue(categories) });
    renderWithClient(
      <Main
        onLocked={vi.fn()}
        subscribeAutoType={(on) => {
          fire = on;
          return () => undefined;
        }}
      />,
      client,
    );
    act(() => fire!({ window: "Putty", candidates: [{ id: "a", title: "Box", username: null }] }));
    expect(await screen.findByRole("region", { name: "Auto-type" })).toHaveTextContent("Putty");
  });

  it("opens Profile and creates an identity from it", async () => {
    const { user } = setup({ profile: vi.fn().mockResolvedValue({ identity: null, card: null }) });
    await user.click(screen.getByRole("button", { name: "Profile" }));
    await user.click(await screen.findByRole("button", { name: "Create identity" }));
    expect(await screen.findByRole("heading", { name: "New Identity" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(screen.getByRole("button", { name: "Profile" }));
    await user.click(screen.getByRole("button", { name: /All items/ }));
    expect(screen.queryByRole("region", { name: "Profile" })).not.toBeInTheDocument();
  });
});

describe("Main follows the settings", () => {
  it("lists categories in the sidebar and filters by one", async () => {
    const { user } = withSettings({});
    const notes = await screen.findByRole("button", { name: "Secure Notes" });
    await user.click(notes);
    expect(notes).toHaveAttribute("aria-pressed", "true");
    const list = screen.getByRole("list");
    expect(within(list).queryByText("GitHub")).not.toBeInTheDocument();
    await user.click(notes);
    expect(await within(list).findByText("GitHub")).toBeInTheDocument();
  });

  it("hides categories and tags when asked", async () => {
    withSettings({ sidebar_categories: false, sidebar_tags: false }, { tags: vi.fn().mockResolvedValue([{ name: "dev", count: 1 }]) });
    expect(await screen.findByRole("button", { name: /GitHub/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Secure Notes" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^dev/ })).not.toBeInTheDocument();
  });

  it("opens to Favorites", async () => {
    withSettings({ open_to: "favorites" });
    await waitFor(() => expect(screen.getByRole("button", { name: "Favorites" })).toHaveClass("active"));
    const list = screen.getByRole("list");
    await waitFor(() => expect(within(list).queryByText("GitHub")).not.toBeInTheDocument());
    expect(within(list).getByText("Note")).toBeInTheDocument();
  });

  it("opens to Watchtower", async () => {
    withSettings({ open_to: "watchtower" });
    expect(await screen.findByRole("region", { name: "Watchtower" })).toBeInTheDocument();
  });

  it("asks before a program uses an SSH key, one request at a time", async () => {
    const client = fakeClient({
      vaults: vi.fn().mockResolvedValue([personal]),
      items: vi.fn().mockResolvedValue([]),
      settings: vi.fn().mockResolvedValue(DEFAULT_SETTINGS),
    });
    let ask: ((r: { id: number; title: string; fingerprint: string }) => void) | undefined;
    render(
      <ClientProvider client={client}>
        <SettingsProvider>
          <Main onLocked={vi.fn()} copy={vi.fn()} subscribeSshApprove={(fn) => ((ask = fn), () => undefined)} />
        </SettingsProvider>
      </ClientProvider>,
    );
    act(() => {
      ask?.({ id: 1, title: "GitHub", fingerprint: "SHA256:a" });
      ask?.({ id: 2, title: "Work", fingerprint: "SHA256:b" });
    });
    const user = userEvent.setup();
    expect(screen.getByRole("alertdialog")).toHaveTextContent("GitHub");
    await user.click(screen.getByRole("button", { name: "Allow" }));
    expect(await screen.findByRole("alertdialog")).toHaveTextContent("Work");
    await user.click(screen.getByRole("button", { name: "Deny" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(client.sshAnswer).toHaveBeenCalledWith(2, false);
  });
});

describe("Main — copying", () => {
  it("copies through the app, hidden from clipboard history, with the chosen delay", async () => {
    const client = fakeClient({
      vaults: vi.fn().mockResolvedValue([personal]),
      items: vi.fn().mockResolvedValue([summary]),
      item: vi.fn().mockResolvedValue(item),
      settings: vi.fn().mockResolvedValue({ ...DEFAULT_SETTINGS, clipboard_clear_seconds: 30 }),
    });
    render(
      <ClientProvider client={client}>
        <SettingsProvider>
          <Main onLocked={vi.fn()} />
        </SettingsProvider>
      </ClientProvider>,
    );
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /GitHub/ }));
    const copies = await screen.findAllByRole("button", { name: "Copy" });
    await waitFor(async () => {
      await user.click(copies[0]!);
      expect(client.copySecret).toHaveBeenCalledWith(item.username, 30_000);
    });
  });
});
