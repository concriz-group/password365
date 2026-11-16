import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { item } from "../test/fixtures";
import { SignInWithRow } from "./SignInWithRow";

describe("SignInWithRow", () => {
  it("links to the login it points at", async () => {
    const onOpen = vi.fn();
    renderWithClient(<SignInWithRow label="Sign in with" itemId="g1" onOpen={onOpen} />, fakeClient({ item: vi.fn().mockResolvedValue({ ...item, title: "Google" }) }));
    await userEvent.setup().click(await screen.findByRole("button", { name: "Google" }));
    expect(onOpen).toHaveBeenCalledWith("g1");
  });

  it("says when that login is gone", async () => {
    renderWithClient(<SignInWithRow label="Sign in with" itemId="gone" />, fakeClient({ item: vi.fn().mockRejectedValue({ code: "not_found", message: "item not found" }) }));
    expect(await screen.findByText(/was deleted/)).toBeInTheDocument();
  });
});
