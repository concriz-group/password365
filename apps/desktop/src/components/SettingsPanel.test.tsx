import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ClientProvider } from "../api/context";
import { render } from "@testing-library/react";
import { DEFAULT_SETTINGS, SettingsProvider, applyToPage } from "../settings";
import { fakeClient } from "../test/fakeClient";
import { personal, work } from "../test/fixtures";
import { SettingsPanel } from "./SettingsPanel";

function setup(overrides = {}) {
  const client = fakeClient({ settings: vi.fn().mockResolvedValue({ ...DEFAULT_SETTINGS }), ...overrides });
  const onClose = vi.fn();
  render(
    <ClientProvider client={client}>
      <SettingsProvider>
        <SettingsPanel onClose={onClose} vaults={[personal, work]} />
      </SettingsProvider>
    </ClientProvider>,
  );
  return { client, onClose, user: userEvent.setup() };
}

const lastSaved = (client: ReturnType<typeof fakeClient>) => vi.mocked(client.updateSettings).mock.lastCall?.[0];

describe("SettingsPanel", () => {
  it("opens on General: start at login, default vault and first screen", async () => {
    const { client, user } = setup();
    expect(screen.getByRole("button", { name: "General" })).toHaveAttribute("aria-current", "page");
    const login = screen.getByLabelText(/Start Password365/);
    await waitFor(() => expect(login).toBeEnabled());
    await user.click(login);
    expect(lastSaved(client)).toEqual({ ...DEFAULT_SETTINGS, start_at_login: true });
    expect(await screen.findByRole("status")).toHaveTextContent("Saved.");
    await user.selectOptions(screen.getByLabelText("Save new items in"), work.id);
    await user.selectOptions(screen.getByLabelText("Open Password365 to"), "favorites");
    expect(lastSaved(client)).toMatchObject({ start_at_login: true, default_vault: work.id, open_to: "favorites" });
  });

  it("changes theme, density, zoom and the sidebar", async () => {
    const { client, user } = setup();
    await user.click(screen.getByRole("button", { name: "Appearance" }));
    await waitFor(() => expect(screen.getByLabelText("Theme")).toBeEnabled());
    await user.selectOptions(screen.getByLabelText("Theme"), "dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    await user.click(screen.getByLabelText("Compact"));
    expect(document.documentElement.dataset.density).toBe("compact");
    await user.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(screen.getByText("110%")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reset to default" }));
    await user.click(screen.getByRole("button", { name: "Zoom out" }));
    expect(lastSaved(client)).toMatchObject({ zoom_percent: 90 });
    await user.click(screen.getByLabelText("Tags"));
    expect(lastSaved(client)).toMatchObject({ theme: "dark", density: "compact", sidebar_tags: false });
    applyToPage(DEFAULT_SETTINGS);
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it("sets auto-lock, sleep and clipboard clearing", async () => {
    const { client, user } = setup();
    await user.click(screen.getByRole("button", { name: "Security" }));
    const select = screen.getByLabelText("Lock after being idle for");
    await waitFor(() => expect(select).toBeEnabled());
    expect(select).toHaveDisplayValue("10 minutes");
    await user.selectOptions(select, "60");
    await user.click(screen.getByLabelText(/Lock when the computer/));
    await user.selectOptions(screen.getByLabelText(/Remove copied/), "0");
    expect(lastSaved(client)).toMatchObject({ auto_lock_minutes: 60, lock_on_sleep: false, clipboard_clear_seconds: 0 });
    expect(screen.getByRole("option", { name: "30 seconds" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "2 hours" })).toBeInTheDocument();
  });

  it("shows save errors", async () => {
    const { user } = setup({ updateSettings: vi.fn().mockRejectedValue({ code: "internal", message: "boom" }) });
    await waitFor(() => expect(screen.getByLabelText(/Start Password365/)).toBeEnabled());
    await user.click(screen.getByLabelText(/Start Password365/));
    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
  });

  it("changes the master password", async () => {
    const { client, user } = setup();
    await user.click(screen.getByRole("button", { name: "Security" }));
    const submit = screen.getByRole("button", { name: "Change master password" });
    await user.type(screen.getByLabelText("Current master password"), "old password!");
    await user.type(screen.getByLabelText("New master password"), "a new long password");
    expect(submit).toBeDisabled();
    await user.type(screen.getByLabelText("Repeat new master password"), "a new long password");
    await waitFor(() => expect(submit).toBeEnabled());
    await user.click(submit);
    expect(client.changeMasterPassword).toHaveBeenCalledWith("old password!", "a new long password", "a new long password");
    expect(await screen.findByRole("status")).toHaveTextContent("Master password changed.");
    expect(screen.getByLabelText("Current master password")).toHaveValue("");
  });

  it("shows password change errors, the other sections, and closes", async () => {
    const { onClose, user } = setup({
      changeMasterPassword: vi.fn().mockRejectedValue({ code: "wrong_credentials", message: "wrong master password" }),
    });
    await user.click(screen.getByRole("button", { name: "Security" }));
    await user.type(screen.getByLabelText("Current master password"), "x");
    await user.type(screen.getByLabelText("New master password"), "a new long password");
    await user.type(screen.getByLabelText("Repeat new master password"), "a new long password");
    const submit = screen.getByRole("button", { name: "Change master password" });
    await waitFor(() => expect(submit).toBeEnabled());
    await user.click(submit);
    expect(await screen.findByRole("alert")).toHaveTextContent("wrong master password");
    await user.click(screen.getByRole("button", { name: "Backup" }));
    expect(screen.getByText(/Last backup/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "About" }));
    expect(screen.getByText(/GNU GPL/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Advanced" }));
    expect(screen.getByLabelText("Language")).toHaveValue("");
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalled();
  });
});
