import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { EraseVault } from "./EraseVault";

describe("EraseVault", () => {
  it("needs the confirmation word before it erases, then reports it", async () => {
    const eraseVault = vi.fn().mockResolvedValue(undefined);
    const onErased = vi.fn();
    renderWithClient(<EraseVault onErased={onErased} onCancel={vi.fn()} />, fakeClient({ eraseVault }));
    const user = userEvent.setup();
    const button = screen.getByRole("button", { name: "Erase this vault" });
    expect(button).toBeDisabled();
    await user.type(screen.getByLabelText("To go on, type ERASE:"), "erase");
    expect(button).toBeEnabled();
    await user.click(button);
    await waitFor(() => expect(onErased).toHaveBeenCalled());
    expect(eraseVault).toHaveBeenCalledTimes(1);
  });

  it("does nothing when the person keeps the vault", async () => {
    const eraseVault = vi.fn();
    const onCancel = vi.fn();
    renderWithClient(<EraseVault onCancel={onCancel} />, fakeClient({ eraseVault }));
    await userEvent.setup().click(screen.getByRole("button", { name: "Keep it" }));
    expect(onCancel).toHaveBeenCalled();
    expect(eraseVault).not.toHaveBeenCalled();
  });

  it("shows why it failed", async () => {
    const eraseVault = vi.fn().mockRejectedValue({ code: "io", message: "cannot erase the vault" });
    renderWithClient(<EraseVault onCancel={vi.fn()} />, fakeClient({ eraseVault }));
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("To go on, type ERASE:"), "ERASE");
    await user.click(screen.getByRole("button", { name: "Erase this vault" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("cannot erase the vault");
  });
});
