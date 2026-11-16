import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { Settings, UpdateInfo } from "../api/types";
import { SettingsProvider, DEFAULT_SETTINGS } from "../settings";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { UpdateBanner } from "./UpdateBanner";
import { UpdateSettings } from "./UpdateSettings";

function withSettings(update_checks: Settings["update_checks"], extra: Record<string, unknown> = {}) {
  return fakeClient({ settings: vi.fn().mockResolvedValue({ ...DEFAULT_SETTINGS, update_checks }), ...extra });
}

describe("UpdateBanner", () => {
  it("asks once whether to check for updates, and remembers the answer", async () => {
    const client = withSettings("unasked");
    const { unmount } = renderWithClient(<SettingsProvider><UpdateBanner /></SettingsProvider>, client);
    await userEvent.click(await screen.findByRole("button", { name: "Check automatically" }));
    expect(client.updateSettings).toHaveBeenCalledWith(expect.objectContaining({ update_checks: "automatic" }));
    await waitFor(() => expect(screen.queryByRole("note")).not.toBeInTheDocument());
    unmount();

    const other = withSettings("unasked");
    renderWithClient(<SettingsProvider><UpdateBanner /></SettingsProvider>, other);
    await userEvent.click(await screen.findByRole("button", { name: "Not now" }));
    expect(other.updateSettings).toHaveBeenCalledWith(expect.objectContaining({ update_checks: "manual" }));
  });

  it("offers a version an automatic check found, and installs it", async () => {
    const client = withSettings("automatic");
    let emit: (u: UpdateInfo) => void = () => {};
    const subscribe = (on: (u: UpdateInfo) => void) => {
      emit = on;
      return () => {};
    };
    renderWithClient(<SettingsProvider><UpdateBanner subscribeUpdate={subscribe} /></SettingsProvider>, client);
    await waitFor(() => expect(client.settings).toHaveBeenCalled());
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    emit({ version: "0.2.0", notes: null });
    expect(await screen.findByRole("status")).toHaveTextContent("Password365 0.2.0 is available.");
    await userEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    expect(client.updateInstall).toHaveBeenCalled();
  });

  it("lists what changed, and a skipped version stays quiet", async () => {
    const client = withSettings("automatic");
    let emit: (u: UpdateInfo) => void = () => {};
    renderWithClient(
      <SettingsProvider>
        <UpdateBanner subscribeUpdate={(on) => ((emit = on), () => {})} />
      </SettingsProvider>,
      client,
    );
    await waitFor(() => expect(client.settings).toHaveBeenCalled());
    emit({ version: "0.2.0", notes: "### Added\n- Skip a version.\n- What's new." });
    await userEvent.click(await screen.findByText("What's new"));
    expect(screen.getByText("Added")).toBeInTheDocument();
    expect(screen.getByText("• Skip a version.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Skip this version" }));
    expect(client.updateSettings).toHaveBeenCalledWith(expect.objectContaining({ skipped_update: "0.2.0" }));
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("shows an install failure and lets the user try again", async () => {
    const client = withSettings("manual", { updateInstall: vi.fn().mockRejectedValue({ code: "io", message: "Could not install the update: offline" }) });
    let emit: (u: UpdateInfo) => void = () => {};
    renderWithClient(
      <SettingsProvider>
        <UpdateBanner subscribeUpdate={(on) => ((emit = on), () => {})} />
      </SettingsProvider>,
      client,
    );
    await waitFor(() => expect(client.settings).toHaveBeenCalled());
    emit({ version: "0.2.0", notes: null });
    await userEvent.click(await screen.findByRole("button", { name: "Install and restart" }));
    expect(await screen.findByText(/offline/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install and restart" })).toBeEnabled();
  });
});

describe("UpdateSettings", () => {
  it("turns automatic checks on and off", async () => {
    const client = withSettings("manual");
    renderWithClient(<SettingsProvider><UpdateSettings /></SettingsProvider>, client);
    const box = await screen.findByRole("checkbox", { name: "Check for updates automatically" });
    await waitFor(() => expect(box).toBeEnabled());
    expect(box).not.toBeChecked();
    await userEvent.click(box);
    expect(client.updateSettings).toHaveBeenCalledWith(expect.objectContaining({ update_checks: "automatic" }));
  });

  it("checks now: up to date, or a version to install", async () => {
    const updateCheck = vi.fn().mockResolvedValueOnce(null).mockResolvedValueOnce({ version: "0.3.0", notes: null });
    const client = withSettings("manual", { updateCheck });
    renderWithClient(<SettingsProvider><UpdateSettings /></SettingsProvider>, client);
    await userEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    expect(await screen.findByText("Password365 is up to date.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await screen.findByText("Password365 0.3.0 is available.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    expect(client.updateInstall).toHaveBeenCalled();
  });

  it("shows when the check fails", async () => {
    const client = withSettings("manual", { updateCheck: vi.fn().mockRejectedValue({ code: "io", message: "Could not check for updates: no network" }) });
    renderWithClient(<SettingsProvider><UpdateSettings /></SettingsProvider>, client);
    await userEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    expect(await screen.findByText(/no network/)).toBeInTheDocument();
  });
});
