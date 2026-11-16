import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { HelpSection } from "./HelpSection";

describe("HelpSection", () => {
  it("opens each help topic by name (the backend holds the addresses)", async () => {
    const client = fakeClient();
    renderWithClient(<HelpSection />, client);
    for (const [name, topic] of [
      ["Report a bug", "bug"],
      ["Suggest an improvement", "idea"],
      ["Help translate Password365", "translate"],
      ["Report a security problem privately", "security"],
      ["Write to us", "email"],
    ]) {
      await userEvent.click(screen.getByRole("button", { name }));
      expect(client.openHelp).toHaveBeenLastCalledWith(topic);
    }
    expect(screen.getByText(/never post security problems in public/)).toBeInTheDocument();
  });

  it("says when the page cannot be opened", async () => {
    const client = fakeClient({ openHelp: vi.fn().mockRejectedValue({ code: "io", message: "Could not open the page: no browser" }) });
    renderWithClient(<HelpSection />, client);
    await userEvent.click(screen.getByRole("button", { name: "Report a bug" }));
    expect(await screen.findByText(/no browser/)).toBeInTheDocument();
  });
});
