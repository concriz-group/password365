import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { DuplicateGroup } from "../api/types";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { DuplicatesPanel } from "./DuplicatesPanel";

const member = (id: string, title: string, over = {}) => ({
  id,
  title,
  urls: [`https://${title}`],
  updated_at: Date.UTC(2026, 10, 1),
  password_group: 0,
  has_totp: false,
  has_notes: false,
  history: 0,
  ...over,
});

const group: DuplicateGroup = {
  site: "amazon.es",
  username: "ana",
  same_password: false,
  members: [member("a", "amazon.es", { has_notes: true }), member("b", "amazon.com", { password_group: 1 }), member("c", "amazon.com.br")],
};

function setup(groups: DuplicateGroup[][] = [[group], []]) {
  const duplicateGroups = vi.fn();
  groups.forEach((g) => duplicateGroups.mockResolvedValueOnce(g));
  const client = fakeClient({ duplicateGroups, mergeDuplicates: vi.fn().mockResolvedValue(2) });
  const onMerged = vi.fn();
  const onOpenItem = vi.fn();
  renderWithClient(<DuplicatesPanel onOpenItem={onOpenItem} onMerged={onMerged} />, client);
  return { client, onMerged, onOpenItem, user: userEvent.setup() };
}

describe("DuplicatesPanel", () => {
  it("shows each group with the suggested login selected and warns when passwords differ", async () => {
    setup();
    expect(await screen.findByRole("heading", { name: /amazon\.es · ana/ })).toBeInTheDocument();
    expect(screen.getByText("The passwords are different: check before merging.")).toBeInTheDocument();
    expect(screen.getAllByRole("radio")[0]).toBeChecked();
    expect(screen.getByText("Suggested")).toBeInTheDocument();
  });

  it("merges the others into the one chosen, then reloads", async () => {
    const { client, onMerged, user } = setup();
    await screen.findByRole("heading", { name: /amazon\.es/ });
    await user.click(screen.getAllByRole("radio")[1]!);
    await user.click(screen.getByRole("button", { name: /Merge into the selected login \(2 to Recently Deleted\)/ }));
    expect(client.mergeDuplicates).toHaveBeenCalledWith("b", ["a", "c"]);
    await waitFor(() => expect(onMerged).toHaveBeenCalled());
    expect(await screen.findByText("No duplicates found.")).toBeInTheDocument();
  });

  it("keeps the suggested one when nothing else is chosen, and can open a login", async () => {
    const { client, onOpenItem, user } = setup();
    await screen.findByRole("heading", { name: /amazon\.es/ });
    await user.click(screen.getAllByRole("button", { name: "Open" })[2]!);
    expect(onOpenItem).toHaveBeenCalledWith("c");
    await user.click(screen.getByRole("button", { name: /Merge into/ }));
    expect(client.mergeDuplicates).toHaveBeenCalledWith("a", ["b", "c"]);
  });

  it("says when there are none, and shows a merge error without losing the list", async () => {
    setup([[]]);
    expect(await screen.findByText("No duplicates found.")).toBeInTheDocument();
  });

  it("shows why a merge failed", async () => {
    const client = fakeClient({
      duplicateGroups: vi.fn().mockResolvedValue([group]),
      mergeDuplicates: vi.fn().mockRejectedValue({ code: "validation", message: "these logins are not the same account" }),
    });
    renderWithClient(<DuplicatesPanel onOpenItem={vi.fn()} onMerged={vi.fn()} />, client);
    await userEvent.setup().click(await screen.findByRole("button", { name: /Merge into/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("these logins are not the same account");
    expect(screen.getByRole("heading", { name: /amazon\.es/ })).toBeInTheDocument();
  });
});
