import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { FilePicker } from "../lib/files";
import { fakeFiles } from "../test/fakeFiles";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { personal, work } from "../test/fixtures";
import { summaryLines, Transfer } from "./Transfer";

const SUMMARY = { imported: 2, identical: 1, changed: 1, updated: 0, duplicates_in_file: 0, skipped: [[4, "no username, password or URL"]] };

function setup(files: Partial<FilePicker> = {}, overrides = {}) {
  const client = fakeClient({
    importCsv: vi.fn().mockResolvedValue(SUMMARY),
    exportCsv: vi.fn().mockResolvedValue(3),
    ...overrides,
  });
  const picker = fakeFiles(files);
  const props = { vaults: [personal, work], files: picker, onImported: vi.fn(), onClose: vi.fn() };
  renderWithClient(<Transfer {...props} />, client);
  return { client, picker, props, user: userEvent.setup() };
}

describe("Transfer — import", () => {
  it("imports the picked file into the chosen vault and shows a summary", async () => {
    const { client, props, user } = setup();
    expect(screen.getByRole("heading", { name: "Migrate your data" })).toBeInTheDocument();
    await user.type(screen.getByLabelText(/Where are your passwords/), "pass");
    expect(screen.queryByRole("button", { name: /Chrome/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Proton Pass/ }));
    expect(screen.getByText(/In Proton Pass, open Settings/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Choose another source" }));
    await user.clear(screen.getByLabelText(/Where are your passwords/));
    await user.type(screen.getByLabelText(/Where are your passwords/), "zzz");
    expect(screen.getByText(/No match/)).toBeInTheDocument();
    await user.clear(screen.getByLabelText(/Where are your passwords/));
    await user.click(screen.getByRole("button", { name: "A different password manager" }));
    await user.selectOptions(screen.getByLabelText("Into vault"), "v2");
    await user.click(screen.getByLabelText(/Update passwords/));
    await user.click(screen.getByRole("button", { name: "Choose the CSV file and import…" }));
    expect(client.importCsv).toHaveBeenCalledWith("/tmp/in.csv", true, "v2");
    expect(await screen.findByText(/Imported 2/)).toBeInTheDocument();
    expect(screen.getByText("Skipped line 4: no username, password or URL")).toBeInTheDocument();
    expect(screen.getByText(/delete it now/)).toBeInTheDocument();
    expect(props.onImported).toHaveBeenCalled();
  });

  it("does nothing when the dialog is cancelled", async () => {
    const { client, user } = setup({ openCsv: vi.fn().mockResolvedValue(null) });
    await user.click(screen.getByRole("button", { name: /Chrome/ }));
    await user.click(screen.getByRole("button", { name: "Choose the CSV file and import…" }));
    expect(client.importCsv).not.toHaveBeenCalled();
  });

  it("shows import errors", async () => {
    const { user } = setup({}, { importCsv: vi.fn().mockRejectedValue({ code: "validation", message: "not a password CSV" }) });
    await user.click(screen.getByRole("button", { name: "CSV file" }));
    await user.click(screen.getByRole("button", { name: "Choose the CSV file and import…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("not a password CSV");
  });
});

describe("Transfer — 1Password", () => {
  it("takes a .1pux file from 1Password", async () => {
    const { client, user } = setup({}, { import1pux: vi.fn().mockResolvedValue({ imported: 5, identical: 0, changed: 0, updated: 0, duplicates_in_file: 0, skipped: [] }) });
    await user.click(screen.getByRole("button", { name: /1Password/ }));
    expect(screen.getByText(/1PUX format/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Choose the export file and import…" }));
    expect(client.import1pux).toHaveBeenCalledWith("/tmp/export.1pux", "v1");
    expect(await screen.findByText(/Imported 5/)).toBeInTheDocument();
  });
});

describe("Transfer — Bitwarden", () => {
  it("takes Bitwarden's JSON export", async () => {
    const { client, picker, user } = setup({}, { importBitwarden: vi.fn().mockResolvedValue({ imported: 2, identical: 0, changed: 0, updated: 0, duplicates_in_file: 0, skipped: [] }) });
    vi.mocked(picker.openImport).mockResolvedValue("/tmp/bitwarden.json");
    await user.click(screen.getByRole("button", { name: /Bitwarden/ }));
    await user.click(screen.getByRole("button", { name: "Choose the export file and import…" }));
    expect(client.importBitwarden).toHaveBeenCalledWith("/tmp/bitwarden.json", "v1");
  });
});

describe("Transfer — CXF", () => {
  it("imports a credential exchange file", async () => {
    const { client, picker, user } = setup({}, { importCxf: vi.fn().mockResolvedValue({ imported: 4, identical: 0, changed: 0, updated: 0, duplicates_in_file: 0, skipped: [] }) });
    vi.mocked(picker.openImport).mockResolvedValue("/tmp/export.json");
    await user.click(screen.getByRole("button", { name: /Credential exchange file/ }));
    expect(screen.getByText(/Choose a CXF file/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Choose the export file and import…" }));
    expect(client.importCxf).toHaveBeenCalledWith("/tmp/export.json", "v1");
    expect(client.importBitwarden).not.toHaveBeenCalled();
  });
});

describe("Transfer — KeePass", () => {
  it("opens a .kdbx with its password", async () => {
    const { client, picker, user } = setup({}, { importKdbx: vi.fn().mockResolvedValue({ imported: 3, identical: 0, changed: 0, updated: 0, duplicates_in_file: 0, skipped: [] }) });
    vi.mocked(picker.openImport).mockResolvedValue("/tmp/vault.kdbx");
    await user.click(screen.getByRole("button", { name: /KeePassXC/ }));
    await user.type(screen.getByLabelText(/KeePass database password/, { selector: "input" }), "kp-pass");
    await user.click(screen.getByRole("button", { name: "Choose the export file and import…" }));
    expect(client.importKdbx).toHaveBeenCalledWith("/tmp/vault.kdbx", "kp-pass", "v1", undefined);
    expect(screen.getByLabelText(/KeePass database password/, { selector: "input" })).toHaveValue("");
  });

  it("can add a key file", async () => {
    const { client, picker, user } = setup({}, { importKdbx: vi.fn().mockResolvedValue({ imported: 1, identical: 0, changed: 0, updated: 0, duplicates_in_file: 0, skipped: [] }) });
    vi.mocked(picker.openImport).mockResolvedValue("/tmp/vault.kdbx");
    vi.mocked(picker.openAnyFile).mockResolvedValue("/keys/vault.keyx");
    await user.click(screen.getByRole("button", { name: /KeePassXC/ }));
    await user.click(screen.getByRole("button", { name: "Key file (optional)…" }));
    expect(screen.getByText("vault.keyx")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Choose the export file and import…" }));
    expect(client.importKdbx).toHaveBeenCalledWith("/tmp/vault.kdbx", "", "v1", "/keys/vault.keyx");
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(screen.queryByText("vault.keyx")).not.toBeInTheDocument();
  });
});

describe("Transfer — close", () => {
  it("closes", async () => {
    const { props, user } = setup();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(props.onClose).toHaveBeenCalled();
  });
});

describe("Transfer — summary", () => {
  const base = { imported: 0, identical: 0, changed: 0, updated: 0, duplicates_in_file: 0, skipped: [] as [number, string][] };

  it("says plainly when nothing was new", () => {
    expect(summaryLines({ ...base, identical: 4 })).toEqual([["nothingNew", 4]]);
    expect(summaryLines(base)).toEqual([["nothingToImport", 0]]);
    expect(summaryLines({ ...base, skipped: [[3, "bad"]] })).toEqual([]);
  });

  it("lists only what happened", () => {
    expect(summaryLines({ ...base, imported: 2, identical: 1, changed: 1 })).toEqual([
      ["importedCount", 2],
      ["changedCount", 1],
      ["identicalCount", 1],
    ]);
  });

  it("shows the summary and hides the CSV-only option for full exports", async () => {
    const { picker, user } = setup({}, { import1pux: vi.fn().mockResolvedValue({ ...base, identical: 4 }) });
    vi.mocked(picker.openImport).mockResolvedValue("/tmp/export.1pux");
    await user.click(screen.getByRole("button", { name: /1Password/ }));
    expect(screen.queryByLabelText(/Update passwords/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Choose the export file and import…" }));
    expect(await screen.findByText("Nothing new: all 4 items were already in the vault.")).toBeInTheDocument();
    expect(screen.getByText(/The file you imported contains your passwords/)).toBeInTheDocument();
  });
});

