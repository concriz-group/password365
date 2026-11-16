import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { fakeFiles } from "../test/fakeFiles";
import { Onboarding } from "./Onboarding";

const KEY = "Q1-AAAAAA-BBBBB-CCCCC-DDDDD-EEEEE-FFFFF";

describe("Restore from a backup", () => {
  async function openRestore(overrides = {}, files = fakeFiles()) {
    const user = userEvent.setup();
    const onDone = vi.fn();
    const client = fakeClient(overrides);
    renderWithClient(<Onboarding onDone={onDone} files={files} />, client);
    await user.click(screen.getByRole("button", { name: "restore from a backup" }));
    return { user, onDone, client, files };
  }

  it("restores with the file, Secret Key and master password", async () => {
    const { user, onDone, client } = await openRestore();
    const submit = screen.getByRole("button", { name: "Restore" });
    expect(submit).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Choose backup file…" }));
    expect(screen.getByLabelText("Backup file")).toHaveTextContent("/tmp/old.qlbackup");
    await user.type(screen.getByLabelText("Secret Key"), ` ${KEY} `);
    await user.type(screen.getByLabelText("Master password"), "my master password");
    await user.click(submit);
    expect(client.restoreBackup).toHaveBeenCalledWith("/tmp/old.qlbackup", "my master password", KEY);
    expect(onDone).toHaveBeenCalled();
  });

  it("shows errors, keeps the chosen file when the dialog is cancelled, and goes back", async () => {
    const files = fakeFiles({ openBackup: vi.fn().mockResolvedValueOnce("/tmp/a.qlbackup").mockResolvedValueOnce(null) });
    const restoreBackup = vi.fn().mockRejectedValue({ code: "wrong_credentials", message: "wrong master password or Secret Key" });
    const { user, onDone } = await openRestore({ restoreBackup }, files);
    await user.click(screen.getByRole("button", { name: "Choose backup file…" }));
    await user.click(screen.getByRole("button", { name: "Choose backup file…" }));
    expect(screen.getByLabelText("Backup file")).toHaveTextContent("/tmp/a.qlbackup");
    await user.type(screen.getByLabelText("Secret Key"), KEY);
    await user.type(screen.getByLabelText("Master password"), "nope");
    await user.click(screen.getByRole("button", { name: "Restore" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("wrong master password or Secret Key");
    expect(onDone).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("button", { name: "Create account" })).toBeInTheDocument();
  });
});
