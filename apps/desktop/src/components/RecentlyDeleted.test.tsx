import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { DeletedItem } from "../api/types";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { daysLeft, RecentlyDeleted } from "./RecentlyDeleted";

const DAY = 24 * 60 * 60 * 1000;

function deleted(id: string, title: string): DeletedItem {
  const now = Date.now();
  return {
    id,
    vault_id: "v",
    kind: "login",
    title,
    username: "me",
    urls: [],
    favorite: false,
    tags: [],
    archived: false,
    updated_at: now,
    needs_review: false,
    deleted_at: now,
    purge_at: now + 30 * DAY,
  };
}

describe("daysLeft", () => {
  it("rounds up and never goes negative", () => {
    expect(daysLeft(10 * DAY + 1, 0)).toBe(11);
    expect(daysLeft(0, 5)).toBe(0);
  });
});

describe("RecentlyDeleted", () => {
  it("says when there is nothing to show", async () => {
    renderWithClient(<RecentlyDeleted onRestored={vi.fn()} />, fakeClient());
    expect(await screen.findByText("No recently deleted items.")).toBeInTheDocument();
  });

  it("restores an item and tells the parent", async () => {
    const deletedItems = vi
      .fn()
      .mockResolvedValueOnce([deleted("a", "Mail")])
      .mockResolvedValueOnce([]);
    const client = fakeClient({ deletedItems });
    const onRestored = vi.fn();
    renderWithClient(<RecentlyDeleted onRestored={onRestored} />, client);
    expect(await screen.findByText("me · 30 days left")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Restore" }));
    await waitFor(() => expect(onRestored).toHaveBeenCalled());
    expect(client.restoreItem).toHaveBeenCalledWith("a");
    expect(await screen.findByText("No recently deleted items.")).toBeInTheDocument();
  });

  it("deletes permanently only after confirming", async () => {
    const deletedItems = vi.fn().mockResolvedValue([deleted("a", "Mail")]);
    const client = fakeClient({ deletedItems });
    renderWithClient(<RecentlyDeleted onRestored={vi.fn()} />, client);
    await userEvent.click(await screen.findByRole("button", { name: "Delete Mail permanently" }));
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(client.deletePermanently).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Delete Mail permanently" }));
    await userEvent.click(screen.getByRole("button", { name: "Delete permanently" }));
    await waitFor(() => expect(client.deletePermanently).toHaveBeenCalledWith("a"));
  });

  it("shows errors", async () => {
    const client = fakeClient({
      deletedItems: vi.fn().mockRejectedValue({ code: "locked", message: "Vault is locked" }),
    });
    renderWithClient(<RecentlyDeleted onRestored={vi.fn()} />, client);
    expect(await screen.findByText(/Vault is locked/)).toBeInTheDocument();
  });
});
