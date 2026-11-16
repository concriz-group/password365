import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { PasswordUpdateBanner } from "./PasswordUpdateBanner";

const pending = { device_name: "Mac", changed_at: 1 };

describe("PasswordUpdateBanner", () => {
  it("stays hidden when nothing changed elsewhere", async () => {
    const client = fakeClient({ pendingPasswordChange: vi.fn().mockResolvedValue(null) });
    const { container } = renderWithClient(<PasswordUpdateBanner />, client);
    await vi.waitFor(() => expect(client.pendingPasswordChange).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("asks for the new password and confirms", async () => {
    const client = fakeClient({ pendingPasswordChange: vi.fn().mockResolvedValue(pending) });
    renderWithClient(<PasswordUpdateBanner />, client);
    const user = userEvent.setup();
    expect(await screen.findByText(/changed on Mac/)).toBeInTheDocument();
    await user.type(screen.getByLabelText("New master password"), "new pw");
    await user.click(screen.getByRole("button", { name: "Update" }));
    expect(client.adoptMasterPassword).toHaveBeenCalledWith("new pw");
    expect(await screen.findByRole("status")).toHaveTextContent("now uses your new master password");
    await user.click(screen.getByRole("button", { name: "OK" }));
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("says when the password is not the new one", async () => {
    const client = fakeClient({
      pendingPasswordChange: vi.fn().mockResolvedValue(pending),
      adoptMasterPassword: vi.fn().mockRejectedValue({ code: "wrong_credentials", message: "wrong master password" }),
    });
    renderWithClient(<PasswordUpdateBanner />, client);
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText("New master password"), "old");
    await user.click(screen.getByRole("button", { name: "Update" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("wrong master password");
  });

  it("hides on errors", async () => {
    const client = fakeClient({ pendingPasswordChange: vi.fn().mockRejectedValue({ code: "locked", message: "x" }) });
    const { container } = renderWithClient(<PasswordUpdateBanner />, client);
    await vi.waitFor(() => expect(client.pendingPasswordChange).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });
});
