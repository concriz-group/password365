import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { AutoTypeChooser } from "./AutoTypeChooser";

describe("AutoTypeChooser", () => {
  it("types the chosen item and closes", async () => {
    const onDone = vi.fn();
    const client = fakeClient();
    renderWithClient(
      <AutoTypeChooser
        choice={{
          window: "srv1 - Remote Desktop",
          candidates: [
            { id: "a", title: "Server A", username: "root" },
            { id: "b", title: "Server B", username: null },
          ],
        }}
        onDone={onDone}
      />,
      client,
    );
    expect(screen.getByText("srv1 - Remote Desktop")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Server B/ }));
    expect(client.autoType).toHaveBeenCalledWith("b");
    await waitFor(() => expect(onDone).toHaveBeenCalled());
  });

  it("explains how to set up a window nothing matches, and cancels", async () => {
    const onDone = vi.fn();
    renderWithClient(<AutoTypeChooser choice={{ window: "Notes", candidates: [] }} onDone={onDone} />, fakeClient());
    expect(screen.getByText(/No item is set up for/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onDone).toHaveBeenCalled();
  });

  it("shows typing errors", async () => {
    const client = fakeClient({ autoType: vi.fn().mockRejectedValue({ code: "io", message: "typing failed" }) });
    renderWithClient(
      <AutoTypeChooser choice={{ window: "W", candidates: [{ id: "a", title: "A", username: null }] }} onDone={vi.fn()} />,
      client,
    );
    await userEvent.click(screen.getByRole("button", { name: /A/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("typing failed");
  });
});
