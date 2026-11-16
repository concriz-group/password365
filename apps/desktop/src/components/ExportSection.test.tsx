import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { FilePicker } from "../lib/files";
import { fakeFiles } from "../test/fakeFiles";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { personal, work } from "../test/fixtures";
import { ExportSection } from "./ExportSection";

function setup(files: Partial<FilePicker> = {}, overrides = {}) {
  const client = fakeClient({ exportCsv: vi.fn().mockResolvedValue(3), ...overrides });
  renderWithClient(<ExportSection vaults={[personal, work]} files={fakeFiles(files)} />, client);
  return { client, user: userEvent.setup() };
}

describe("ExportSection", () => {
  it("requires acknowledging the plaintext warning", async () => {
    const { client, user } = setup();
    const button = screen.getByRole("button", { name: "Export CSV…" });
    expect(button).toBeDisabled();
    await user.click(screen.getByLabelText(/not encrypted/));
    await user.type(screen.getByLabelText(/Master password/), "pw");
    await user.click(button);
    expect(client.exportCsv).toHaveBeenCalledWith("/tmp/out.csv", undefined, "pw");
    expect(await screen.findByText("Exported 3 logins.")).toBeInTheDocument();
  });

  it("exports every item as CXF after the same warning", async () => {
    const { client, user } = setup({ saveFile: vi.fn().mockResolvedValue("/tmp/out.cxf.json") }, { exportCxf: vi.fn().mockResolvedValue(5) });
    const button = screen.getByRole("button", { name: "Export CXF…" });
    expect(button).toBeDisabled();
    await user.click(screen.getByLabelText(/not encrypted/));
    await user.type(screen.getByLabelText(/Master password/), "pw");
    await user.click(button);
    expect(client.exportCxf).toHaveBeenCalledWith("/tmp/out.cxf.json", undefined, "pw");
    expect(await screen.findByText("Exported 5 items.")).toBeInTheDocument();
  });

  it("exports one vault", async () => {
    const { client, user } = setup();
    await user.selectOptions(screen.getByLabelText("Vault"), "v1");
    await user.click(screen.getByLabelText(/not encrypted/));
    await user.type(screen.getByLabelText(/Master password/), "pw");
    await user.click(screen.getByRole("button", { name: "Export CSV…" }));
    expect(client.exportCsv).toHaveBeenCalledWith("/tmp/out.csv", "v1", "pw");
  });

  it("handles cancel and errors", async () => {
    const cancelled = setup({ saveCsv: vi.fn().mockResolvedValue(null) });
    await cancelled.user.click(screen.getByLabelText(/not encrypted/));
    await cancelled.user.click(screen.getByRole("button", { name: "Export CSV…" }));
    expect(cancelled.client.exportCsv).not.toHaveBeenCalled();
  });

  it("shows export errors", async () => {
    const { user } = setup({}, { exportCsv: vi.fn().mockRejectedValue({ code: "io", message: "already exists" }) });
    await user.click(screen.getByLabelText(/not encrypted/));
    await user.type(screen.getByLabelText(/Master password/), "pw");
    await user.click(screen.getByRole("button", { name: "Export CSV…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("already exists");
  });
});
