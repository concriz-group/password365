import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { EmergencyKitSection } from "./EmergencyKitSection";

describe("EmergencyKitSection", () => {
  it("asks for the master password, prints the kit and forgets the Secret Key", async () => {
    let shownWhilePrinting = "";
    const client = fakeClient({
      revealSecretKey: vi.fn().mockResolvedValue("P365-ABC"),
      printPage: vi.fn(async () => {
        shownWhilePrinting = screen.getByLabelText("Emergency Kit").textContent ?? "";
      }),
    });
    renderWithClient(<EmergencyKitSection />, client);
    const user = userEvent.setup();
    expect(screen.getByRole("button", { name: "Print Emergency Kit" })).toBeDisabled();
    await user.type(screen.getByLabelText("Master password"), "pw");
    await user.click(screen.getByRole("button", { name: "Print Emergency Kit" }));
    await waitFor(() => expect(client.printPage).toHaveBeenCalled());
    expect(client.revealSecretKey).toHaveBeenCalledWith("pw");
    expect(shownWhilePrinting).toContain("P365-ABC");
    await waitFor(() => expect(screen.queryByLabelText("Emergency Kit")).not.toBeInTheDocument());
    expect(screen.getByLabelText("Master password")).toHaveValue("");
  });

  it("shows a wrong password", async () => {
    const client = fakeClient({
      revealSecretKey: vi.fn().mockRejectedValue({ code: "wrong_credentials", message: "wrong master password" }),
    });
    renderWithClient(<EmergencyKitSection />, client);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("Master password"), "nope");
    await user.click(screen.getByRole("button", { name: "Print Emergency Kit" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("wrong master password");
    expect(client.printPage).not.toHaveBeenCalled();
  });

  it("keeps the kit in the vault as a document", async () => {
    const saveEmergencyKit = vi.fn().mockResolvedValue("item-1");
    renderWithClient(<EmergencyKitSection />, fakeClient({ saveEmergencyKit }));
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("Master password"), "pw");
    await user.click(screen.getByRole("button", { name: "Save in the vault" }));
    expect(await screen.findByText("Saved in your vault as a document.")).toBeInTheDocument();
    expect(saveEmergencyKit).toHaveBeenCalledWith("pw", expect.objectContaining({ title: "Password365 Emergency Kit" }));
    expect(screen.getByLabelText("Master password")).toHaveValue("");
  });
});
