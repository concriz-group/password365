import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { Watchtower } from "../api/types";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { WatchtowerPanel } from "./WatchtowerPanel";

const a = { id: "a", title: "Alpha", kind: "login" };
const b = { id: "b", title: "Beta", kind: "login" };
const c = { id: "c", title: "Visa", kind: "card" };

const report: Watchtower = {
  score: 640,
  rating: "good",
  strength: { strong: 3, good: 1, weak: 1 },
  weak: [a],
  reused: [[a, b]],
  unsecured: [],
  expiring: [{ item: c, date: "2026-10", expired: true }],
};

function setup(value: Watchtower | Promise<never> = report) {
  const onOpenItem = vi.fn();
  const watchtower = value instanceof Promise ? vi.fn().mockReturnValue(value) : vi.fn().mockResolvedValue(value);
  const client = fakeClient({ watchtower });
  renderWithClient(<WatchtowerPanel onOpenItem={onOpenItem} />, client);
  return { onOpenItem, user: userEvent.setup() };
}

describe("WatchtowerPanel", () => {
  it("shows the score, the strength bar and only the problems found", async () => {
    setup();
    expect(await screen.findByRole("img", { name: "Score 640, Good" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "3 strong, 1 good, 1 weak" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Weak passwords" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Reused passwords" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Expiring items" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Unsecured websites" })).not.toBeInTheDocument();
  });

  it("lists a finding's items and opens one", async () => {
    const { onOpenItem, user } = setup();
    const buttons = await screen.findAllByRole("button", { name: "Show items" });
    await user.click(buttons[1]!);
    const list = screen.getByRole("list", { name: "Reused passwords" });
    expect(list).toHaveTextContent("AlphaGroup 1BetaGroup 1");
    await user.click(screen.getByRole("button", { name: /Beta/ }));
    expect(onOpenItem).toHaveBeenCalledWith("b");
    await user.click(screen.getAllByRole("button", { name: "Show items" })[1]!);
    expect(screen.getByRole("list", { name: "Expiring items" })).toHaveTextContent("Expired 2026-10");
    await user.click(screen.getByRole("button", { name: "Hide items" }));
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
  });

  it("says when nothing was found and skips the score without passwords", async () => {
    setup({ ...report, score: null, rating: null, strength: { strong: 0, good: 0, weak: 0 }, weak: [], reused: [], expiring: [] });
    expect(await screen.findByText("No problems found.")).toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("shows errors", async () => {
    setup(Promise.reject({ code: "locked", message: "Vault is locked" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Vault is locked"));
  });
});
