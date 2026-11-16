import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { Device } from "../api/types";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { personal, work } from "../test/fixtures";
import { fakeFiles } from "../test/fakeFiles";
import { AddDevice } from "./AddDevice";
import { DevicesPanel, syncFileName } from "./DevicesPanel";

const phone: Device = {
  id: "d1",
  name: "Phone",
  paired_at: 0,
  last_sync: null,
  pending_changes: 2,
  addr: "10.0.0.2:47480",
  out_of_sync: false,
};

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("DevicesPanel", () => {
  function setup(overrides = {}, files = fakeFiles()) {
    const client = fakeClient({
      thisDevice: vi.fn().mockResolvedValue({ id: "me", name: "Laptop" }),
      devices: vi.fn().mockResolvedValue([phone]),
      renameThisDevice: vi.fn().mockResolvedValue({ id: "me", name: "Desk" }),
      syncDevice: vi.fn().mockResolvedValue({ device: phone, changed: 3, conflicts: ["i1"] }),
      ...overrides,
    });
    const onSynced = vi.fn();
    renderWithClient(<DevicesPanel vaults={[personal]} onSynced={onSynced} onClose={vi.fn()} files={files} />, client);
    return { client, onSynced, files, user: userEvent.setup() };
  }

  it("lists paired devices with their sync status", async () => {
    setup();
    expect(await screen.findByText("Phone")).toBeInTheDocument();
    expect(screen.getByText("Last synced never · 2 changes from here not on it yet")).toBeInTheDocument();
    expect(screen.getByLabelText("This device's name")).toHaveValue("Laptop");
  });

  it("says when there are no other devices", async () => {
    setup({ devices: vi.fn().mockResolvedValue([]) });
    expect(await screen.findByText("No other devices yet.")).toBeInTheDocument();
  });

  it("renames this device", async () => {
    const { client, user } = setup();
    const input = await screen.findByLabelText("This device's name");
    await waitFor(() => expect(input).toHaveValue("Laptop"));
    expect(screen.getByRole("button", { name: "Rename" })).toBeDisabled();
    await user.clear(input);
    await user.type(input, "Desk");
    await user.click(screen.getByRole("button", { name: "Rename" }));
    expect(client.renameThisDevice).toHaveBeenCalledWith("Desk");
  });

  it("syncs now and reports changes and conflicts", async () => {
    const { client, onSynced, user } = setup();
    await user.click(await screen.findByRole("button", { name: "Sync now" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Synced with Phone: 3 changes received. 1 item had two different passwords",
    );
    expect(client.syncDevice).toHaveBeenCalledWith("d1");
    expect(onSynced).toHaveBeenCalled();
  });

  it("does not refresh items when nothing changed", async () => {
    const { onSynced, user } = setup({
      syncDevice: vi.fn().mockResolvedValue({ device: phone, changed: 0, conflicts: [] }),
    });
    await user.click(await screen.findByRole("button", { name: "Sync now" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Synced with Phone: 0 changes received.");
    expect(onSynced).not.toHaveBeenCalled();
  });

  it("shows sync errors", async () => {
    const { user } = setup({
      syncDevice: vi.fn().mockRejectedValue({ code: "io", message: "Could not reach the other device" }),
    });
    await user.click(await screen.findByRole("button", { name: "Sync now" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not reach the other device");
  });

  it("saves a sync file for a device", async () => {
    const { client, files, user } = setup({ exportSyncFile: vi.fn().mockResolvedValue(4) });
    await user.click(await screen.findByRole("button", { name: "Save a sync file for Phone" }));
    expect(files.saveSyncFile).toHaveBeenCalledWith("Password365 sync for Phone.p365sync");
    expect(client.exportSyncFile).toHaveBeenCalledWith("d1", "/tmp/out.p365sync");
    expect(await screen.findByRole("status")).toHaveTextContent("Saved 4 changes for Phone.");
  });

  it("imports a sync file", async () => {
    const { client, onSynced, user } = setup({
      importSyncFile: vi.fn().mockResolvedValue({ device: phone, changed: 2, conflicts: [] }),
    });
    await user.click(await screen.findByRole("button", { name: "import a sync file" }));
    expect(client.importSyncFile).toHaveBeenCalledWith("/tmp/in.p365sync");
    expect(await screen.findByRole("status")).toHaveTextContent("Imported from Phone: 2 changes received.");
    expect(onSynced).toHaveBeenCalled();
  });

  it("does nothing when the file dialogs are cancelled", async () => {
    const files = fakeFiles({
      saveSyncFile: vi.fn().mockResolvedValue(null),
      openSyncFile: vi.fn().mockResolvedValue(null),
    });
    const { client, user } = setup({}, files);
    await user.click(await screen.findByRole("button", { name: "Save a sync file for Phone" }));
    await user.click(screen.getByRole("button", { name: "import a sync file" }));
    expect(client.exportSyncFile).not.toHaveBeenCalled();
    expect(client.importSyncFile).not.toHaveBeenCalled();
  });

  it("shows import errors", async () => {
    const { user } = setup({
      importSyncFile: vi.fn().mockRejectedValue({ code: "validation", message: "not a Password365 sync file" }),
    });
    await user.click(await screen.findByRole("button", { name: "import a sync file" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("not a Password365 sync file");
  });

  it("removes a device after confirming", async () => {
    const { client, user } = setup();
    await user.click(await screen.findByRole("button", { name: "Remove Phone" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(client.removeDevice).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Remove Phone" }));
    expect(screen.getByRole("note")).toHaveTextContent("stops syncing with all your devices");
    await user.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(client.removeDevice).toHaveBeenCalledWith("d1"));
  });

  it("opens and closes the add-device flow", async () => {
    const wait = deferred<string>();
    const { client, user } = setup({
      inviteDevice: vi.fn().mockResolvedValue({ code: "PASSWORD365-PAIR:1:ABC", qr_svg: "<svg/>" }),
      waitForNewDevice: vi.fn().mockReturnValue(wait.promise),
    });
    await user.click(await screen.findByRole("button", { name: "Add a device…" }));
    expect(await screen.findByText("PASSWORD365-PAIR:1:ABC")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(client.cancelPairing).toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Add a device…" })).toBeInTheDocument();
  });

  it("reports a newly added device", async () => {
    const { user } = setup({
      inviteDevice: vi.fn().mockResolvedValue({ code: "C", qr_svg: "<svg/>" }),
      waitForNewDevice: vi.fn().mockResolvedValue("123 456"),
      approveNewDevice: vi.fn().mockResolvedValue({ ...phone, name: "Tablet" }),
    });
    await user.click(await screen.findByRole("button", { name: "Add a device…" }));
    await user.click(await screen.findByRole("button", { name: "The codes match — add device" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Tablet was added.");
  });
});

describe("AddDevice", () => {
  function setup(overrides = {}) {
    const client = fakeClient({
      inviteDevice: vi.fn().mockResolvedValue({ code: "CODE", qr_svg: "<svg/>" }),
      waitForNewDevice: vi.fn().mockResolvedValue("123 456"),
      approveNewDevice: vi.fn().mockResolvedValue(phone),
      ...overrides,
    });
    const onAdded = vi.fn();
    const onClose = vi.fn();
    const view = renderWithClient(<AddDevice vaults={[personal, work]} onAdded={onAdded} onClose={onClose} />, client);
    return { client, onAdded, onClose, view, user: userEvent.setup() };
  }

  it("shows the code to compare, lets the user choose vaults and approves", async () => {
    const { client, onAdded, view, user } = setup();
    expect(await screen.findByLabelText("Verification code")).toHaveTextContent("123 456");
    await user.click(screen.getByRole("checkbox", { name: /Work/ }));
    await user.click(screen.getByRole("button", { name: "The codes match — add device" }));
    await waitFor(() => expect(onAdded).toHaveBeenCalledWith(phone));
    expect(client.approveNewDevice).toHaveBeenCalledWith(["v1"]);
    view.unmount();
    expect(client.cancelPairing).not.toHaveBeenCalled();
  });

  it("needs at least one vault", async () => {
    const { user } = setup();
    await screen.findByLabelText("Verification code");
    await user.click(screen.getByRole("checkbox", { name: /Personal/ }));
    await user.click(screen.getByRole("checkbox", { name: /Work/ }));
    expect(screen.getByRole("button", { name: "The codes match — add device" })).toBeDisabled();
    await user.click(screen.getByRole("checkbox", { name: /Work/ }));
    expect(screen.getByRole("button", { name: "The codes match — add device" })).toBeEnabled();
  });

  it("closes when the codes do not match and cancels the pairing", async () => {
    const { client, onClose, view, user } = setup();
    await user.click(await screen.findByRole("button", { name: "They don't match" }));
    expect(onClose).toHaveBeenCalled();
    view.unmount();
    expect(client.cancelPairing).toHaveBeenCalled();
  });

  it("shows the QR code while waiting", async () => {
    const wait = deferred<string>();
    setup({ waitForNewDevice: vi.fn().mockReturnValue(wait.promise) });
    const qr = await screen.findByRole("img", { name: "Pairing QR code" });
    expect(qr.getAttribute("src")).toBe(`data:image/svg+xml;utf8,${encodeURIComponent("<svg/>")}`);
    expect(screen.getByRole("status")).toHaveTextContent("Waiting for the new device");
    await act(async () => wait.resolve("111 222"));
    expect(await screen.findByLabelText("Verification code")).toHaveTextContent("111 222");
  });

  it("copies the pairing code", async () => {
    const wait = deferred<string>();
    const client = fakeClient({
      inviteDevice: vi.fn().mockResolvedValue({ code: "CODE", qr_svg: "<svg/>" }),
      waitForNewDevice: vi.fn().mockReturnValue(wait.promise),
    });
    const copy = vi.fn().mockResolvedValue(undefined);
    renderWithClient(<AddDevice vaults={[personal]} onAdded={vi.fn()} onClose={vi.fn()} copy={copy} />, client);
    await userEvent.setup().click(await screen.findByRole("button", { name: "Copy code" }));
    expect(copy).toHaveBeenCalledWith("CODE");
    expect(await screen.findByRole("button", { name: "Copied" })).toBeInTheDocument();
  });

  it("shows failures and waits for the other side while finishing", async () => {
    const approve = deferred<Device>();
    const { user } = setup({ approveNewDevice: vi.fn().mockReturnValue(approve.promise) });
    await user.click(await screen.findByRole("button", { name: "The codes match — add device" }));
    expect(screen.getByRole("status")).toHaveTextContent("Waiting for the master password on the new device");
    await act(async () => approve.reject({ code: "validation", message: "pairing was cancelled on the other device" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("cancelled on the other device");
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
  });

  it("reports when the invite cannot be opened", async () => {
    setup({ inviteDevice: vi.fn().mockRejectedValue({ code: "io", message: "no network" }) });
    expect(await screen.findByRole("alert")).toHaveTextContent("no network");
  });
});

describe("syncFileName", () => {
  it("keeps names readable and file-system safe", () => {
    expect(syncFileName("Alice's phone")).toBe("Password365 sync for Alices phone.p365sync");
    expect(syncFileName("Café ☕/../x")).toBe("Password365 sync for Café x.p365sync");
    expect(syncFileName("///")).toBe("Password365 sync for device.p365sync");
  });
});
