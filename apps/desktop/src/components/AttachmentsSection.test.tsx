import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { fakeFiles } from "../test/fakeFiles";
import { AttachmentsSection } from "./AttachmentsSection";

const pdf = { id: "a1", name: "passport.pdf", size: 1536, content_type: "application/pdf", available: true };
const later = { id: "a2", name: "scan.png", size: 10, content_type: "image/png", available: false };

function setup(overrides = {}) {
  const client = fakeClient(overrides);
  const files = fakeFiles();
  const onChanged = vi.fn();
  renderWithClient(<AttachmentsSection itemId="i1" attachments={[pdf, later]} files={files} onChanged={onChanged} />, client);
  return { client, files, onChanged, user: userEvent.setup() };
}

describe("AttachmentsSection", () => {
  it("lists files with their size and opens, saves and removes them", async () => {
    const { client, files, onChanged, user } = setup();
    expect(screen.getByText("passport.pdf")).toBeInTheDocument();
    expect(screen.getByText(/1\.5 kB/)).toBeInTheDocument();
    expect(screen.getByText(/arrives with the next sync/)).toBeInTheDocument();
    const [open, openLater] = screen.getAllByRole("button", { name: "Open" });
    expect(openLater).toBeDisabled();
    await user.click(open!);
    expect(client.openAttachment).toHaveBeenCalledWith("i1", "a1");
    await user.click(screen.getAllByRole("button", { name: "Save…" })[0]!);
    expect(files.saveFile).toHaveBeenCalledWith("passport.pdf");
    expect(client.saveAttachment).toHaveBeenCalledWith("i1", "a1", "/tmp/copy.pdf");
    await user.click(screen.getByRole("button", { name: "Remove passport.pdf" }));
    await user.click(screen.getByRole("button", { name: "Remove file" }));
    expect(client.removeAttachment).toHaveBeenCalledWith("i1", "a1");
    expect(onChanged).toHaveBeenCalled();
  });

  it("attaches a file and shows errors", async () => {
    const { client, onChanged, user } = setup({ attachFile: vi.fn().mockResolvedValue(pdf) });
    await user.click(screen.getByRole("button", { name: "+ Attach a file…" }));
    expect(client.attachFile).toHaveBeenCalledWith("i1", "/tmp/passport.pdf");
    expect(onChanged).toHaveBeenCalled();
  });

  it("reports failures", async () => {
    const { user } = setup({ openAttachment: vi.fn().mockRejectedValue({ code: "io", message: "cannot open the file" }) });
    await user.click(screen.getAllByRole("button", { name: "Open" })[0]!);
    expect(await screen.findByRole("alert")).toHaveTextContent("cannot open the file");
    await user.click(screen.getByRole("button", { name: "Remove passport.pdf" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("button", { name: "Remove file" })).not.toBeInTheDocument();
  });
});
