import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { summary } from "../test/fixtures";
import { QuickAccess } from "./QuickAccess";

const bank = { ...summary, id: "b", title: "Bank", username: null };

function setup(overrides = {}) {
  let shown: (() => void) | undefined;
  const client = fakeClient({
    status: vi.fn().mockResolvedValue({ initialized: true, unlocked: true }),
    items: vi.fn().mockResolvedValue([summary, bank]),
    reveal: vi.fn().mockResolvedValue("s3cret"),
    totpCode: vi.fn().mockResolvedValue({ code: "123456", remaining: 20, period: 30 }),
    ...overrides,
  });
  const copy = vi.fn().mockResolvedValue(undefined);
  renderWithClient(
    <QuickAccess
      copy={copy}
      subscribeShown={(fn) => {
        shown = fn;
        return () => undefined;
      }}
    />,
    client,
  );
  return { client, copy, user: userEvent.setup(), show: () => shown?.() };
}

describe("QuickAccess", () => {
  it("searches and copies with the keyboard", async () => {
    const { client, copy, user } = setup();
    const search = await screen.findByRole("searchbox", { name: "Search Password365" });
    expect(await screen.findByRole("option", { name: /GitHub/ })).toHaveAttribute("aria-selected", "true");
    await user.type(search, "git");
    await waitFor(() => expect(client.items).toHaveBeenLastCalledWith("git", undefined));
    await user.keyboard("{Enter}");
    expect(client.reveal).toHaveBeenCalledWith(summary.id, "password");
    expect(copy).toHaveBeenLastCalledWith("s3cret");
    expect(client.hideQuickAccess).toHaveBeenCalled();
    await user.keyboard("{Control>}c{/Control}");
    expect(copy).toHaveBeenLastCalledWith(summary.username);
    await user.keyboard("{Control>}{Alt>}c{/Alt}{/Control}");
    expect(copy).toHaveBeenLastCalledWith("123456");
    await user.keyboard("{Control>}o{/Control}");
    expect(client.openInApp).toHaveBeenCalledWith(summary.id);
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("option", { name: /Bank/ })).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Control>}c{/Control}");
    expect(await screen.findByRole("alert")).toHaveTextContent("This item has no username.");
    await user.keyboard("{ArrowUp}{Escape}");
    expect(client.hideQuickAccess).toHaveBeenCalledTimes(4);
  });

  it("copies with the buttons and hides when it loses focus", async () => {
    const { client, copy, user, show } = setup({ totpCode: vi.fn().mockResolvedValue(null) });
    await screen.findByRole("option", { name: /GitHub/ });
    await user.click(screen.getByRole("button", { name: /Copy password/ }));
    expect(copy).toHaveBeenCalledWith("s3cret");
    await user.click(screen.getByRole("button", { name: /Copy one-time password/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("no one-time password");
    await user.click(screen.getByRole("button", { name: /Copy username/ }));
    await user.click(screen.getByRole("button", { name: /Open in Password365/ }));
    expect(client.openInApp).toHaveBeenCalledWith(summary.id);
    act(() => show());
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    window.dispatchEvent(new Event("blur"));
    expect(client.hideQuickAccess).toHaveBeenCalled();
    await user.dblClick(screen.getByRole("option", { name: /Bank/ }));
    expect(client.openInApp).toHaveBeenLastCalledWith("b");
  });

  it("offers to unlock in the app when locked", async () => {
    const { client, user } = setup({ status: vi.fn().mockResolvedValue({ initialized: true, unlocked: false }) });
    await user.click(await screen.findByRole("button", { name: "Open Password365 to unlock" }));
    expect(client.openInApp).toHaveBeenCalledWith(null);
    expect(client.items).not.toHaveBeenCalled();
  });

  it("shows errors and an empty result", async () => {
    setup({ items: vi.fn().mockResolvedValue([]), reveal: vi.fn().mockRejectedValue({ code: "locked", message: "locked" }) });
    expect(await screen.findByText("No items match.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Copy password/ })).not.toBeInTheDocument();
  });
});
