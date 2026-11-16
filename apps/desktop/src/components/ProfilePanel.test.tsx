import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { summary } from "../test/fixtures";
import { ProfilePanel } from "./ProfilePanel";

const me = { ...summary, id: "me", kind: "identity", title: "Me" };
const visa = { ...summary, id: "visa", kind: "card", title: "Visa" };
const amex = { ...summary, id: "amex", kind: "card", title: "Amex" };

describe("ProfilePanel", () => {
  it("chooses the default card and clears it", async () => {
    const client = fakeClient({
      profile: vi
        .fn()
        .mockResolvedValueOnce({ identity: { id: "me", title: "Me", last4: null }, card: null })
        .mockResolvedValue({ identity: null, card: { id: "amex", title: "Amex", last4: "0005" } }),
      items: vi.fn().mockResolvedValue([me, visa, amex, summary]),
    });
    renderWithClient(<ProfilePanel onCreate={vi.fn()} />, client);
    const card = await screen.findByLabelText("Default payment card");
    expect(screen.getByLabelText("Default identity")).toHaveValue("me");
    expect(card).toHaveValue("");
    expect([...(card as HTMLSelectElement).options].map((o) => o.text)).toEqual(["None", "Visa", "Amex"]);
    await userEvent.selectOptions(card, "amex");
    expect(client.setProfile).toHaveBeenCalledWith("card", "amex");
    await waitFor(() => expect(card).toHaveValue("amex"));
    expect(screen.getByRole("option", { name: "Amex •••• 0005" })).toBeInTheDocument();
    await userEvent.selectOptions(card, "");
    expect(client.setProfile).toHaveBeenLastCalledWith("card", null);
  });

  it("offers to create an identity or card when there is none", async () => {
    const onCreate = vi.fn();
    const client = fakeClient({ profile: vi.fn().mockResolvedValue({ identity: null, card: null }) });
    renderWithClient(<ProfilePanel onCreate={onCreate} />, client);
    await userEvent.click(await screen.findByRole("button", { name: "Create identity" }));
    expect(onCreate).toHaveBeenCalledWith("identity");
    await userEvent.click(screen.getByRole("button", { name: "Add payment card" }));
    expect(onCreate).toHaveBeenLastCalledWith("card");
  });

  it("shows errors", async () => {
    const client = fakeClient({ profile: vi.fn().mockRejectedValue({ code: "locked", message: "Vault is locked" }) });
    renderWithClient(<ProfilePanel onCreate={vi.fn()} />, client);
    expect(await screen.findByRole("alert")).toHaveTextContent("Vault is locked");
  });

  it("shows a failed choice", async () => {
    const client = fakeClient({
      profile: vi.fn().mockResolvedValue({ identity: null, card: null }),
      items: vi.fn().mockResolvedValue([visa]),
      setProfile: vi.fn().mockRejectedValue({ code: "validation", message: "choose a Credit Card item" }),
    });
    renderWithClient(<ProfilePanel onCreate={vi.fn()} />, client);
    await userEvent.selectOptions(await screen.findByLabelText("Default payment card"), "visa");
    expect(await screen.findByRole("alert")).toHaveTextContent("choose a Credit Card item");
  });
});
