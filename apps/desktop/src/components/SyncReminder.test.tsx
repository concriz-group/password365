import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ClientProvider } from "../api/context";
import type { Device } from "../api/types";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { SyncReminder } from "./SyncReminder";

const device = (name: string, out_of_sync: boolean): Device => ({
  id: name,
  name,
  paired_at: 0,
  last_sync: 1,
  pending_changes: 12,
  addr: null,
  out_of_sync,
});

describe("SyncReminder", () => {
  it("names the device that is behind and opens Devices", async () => {
    const onOpen = vi.fn();
    const client = fakeClient({ devices: vi.fn().mockResolvedValue([device("Phone", true), device("Desk", false)]) });
    renderWithClient(<SyncReminder onOpenDevices={onOpen} />, client);
    expect(await screen.findByRole("note")).toHaveTextContent("Phone is missing 12 changes made here.");
    await userEvent.click(screen.getByRole("button", { name: "Devices" }));
    expect(onOpen).toHaveBeenCalled();
  });

  it("says when a newly paired device has not synced yet", async () => {
    const client = fakeClient({
      devices: vi.fn().mockResolvedValue([device("Mac", true), { ...device("New", false), last_sync: null }]),
    });
    renderWithClient(<SyncReminder onOpenDevices={vi.fn()} />, client);
    expect(await screen.findByRole("status")).toHaveTextContent("Syncing with New for the first time");
  });

  it("counts other devices that are behind", async () => {
    const client = fakeClient({
      devices: vi.fn().mockResolvedValue([device("A", true), device("B", true), device("C", true)]),
    });
    renderWithClient(<SyncReminder onOpenDevices={vi.fn()} />, client);
    expect(await screen.findByRole("note")).toHaveTextContent("A (and 2 more devices) is missing");
  });

  it("stays hidden when every device is up to date or on errors", async () => {
    const ok = fakeClient({ devices: vi.fn().mockResolvedValue([device("Phone", false)]) });
    const { unmount } = renderWithClient(<SyncReminder onOpenDevices={vi.fn()} />, ok);
    await waitFor(() => expect(ok.devices).toHaveBeenCalled());
    expect(screen.queryByRole("note")).not.toBeInTheDocument();
    unmount();
    const failing = fakeClient({ devices: vi.fn().mockRejectedValue({ code: "locked", message: "locked" }) });
    renderWithClient(<SyncReminder onOpenDevices={vi.fn()} />, failing);
    await waitFor(() => expect(failing.devices).toHaveBeenCalled());
    expect(screen.queryByRole("note")).not.toBeInTheDocument();
  });

  it("re-checks after a sync", async () => {
    const devices = vi.fn().mockResolvedValueOnce([device("Phone", true)]).mockResolvedValue([]);
    const client = fakeClient({ devices });
    const { rerender } = renderWithClient(<SyncReminder onOpenDevices={vi.fn()} refreshKey={0} />, client);
    expect(await screen.findByRole("note")).toBeInTheDocument();
    rerender(
      <ClientProvider client={client}>
        <SyncReminder onOpenDevices={vi.fn()} refreshKey={1} />
      </ClientProvider>,
    );
    await waitFor(() => expect(screen.queryByRole("note")).not.toBeInTheDocument());
  });
});
