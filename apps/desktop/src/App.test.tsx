import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { App } from "./App";
import { fakeClient, renderWithClient } from "./test/fakeClient";

const status = (initialized: boolean, unlocked: boolean) => vi.fn().mockResolvedValue({ initialized, unlocked });

describe("App routing", () => {
  it("shows onboarding for a new install", async () => {
    renderWithClient(<App />, fakeClient({ status: status(false, false) }));
    expect(await screen.findByText("Create your vault")).toBeInTheDocument();
  });

  it("shows the lock screen when locked", async () => {
    renderWithClient(<App />, fakeClient({ status: status(true, false) }));
    expect(await screen.findByText("Your vault is locked")).toBeInTheDocument();
  });

  it("shows the vault when unlocked and returns to lock screen after Lock", async () => {
    const user = userEvent.setup();
    const statusFn = vi
      .fn()
      .mockResolvedValueOnce({ initialized: true, unlocked: true })
      .mockResolvedValue({ initialized: true, unlocked: false });
    const lock = vi.fn().mockResolvedValue(undefined);
    renderWithClient(<App />, fakeClient({ status: statusFn, lock, vaults: vi.fn().mockResolvedValue([]) }));
    await user.click(await screen.findByRole("button", { name: "Lock" }));
    expect(lock).toHaveBeenCalled();
    expect(await screen.findByText("Your vault is locked")).toBeInTheDocument();
  });

  it("re-checks status when the backend auto-locks", async () => {
    let fire: () => void = () => {};
    const unsubscribe = vi.fn();
    const subscribeStatus = vi.fn((cb: () => void) => {
      fire = cb;
      return unsubscribe;
    });
    const statusFn = vi
      .fn()
      .mockResolvedValueOnce({ initialized: true, unlocked: true })
      .mockResolvedValue({ initialized: true, unlocked: false });
    const { unmount } = renderWithClient(
      <App subscribeStatus={subscribeStatus} />,
      fakeClient({ status: statusFn, vaults: vi.fn().mockResolvedValue([]) }),
    );
    await screen.findByRole("button", { name: "Lock" });
    await act(async () => fire());
    expect(await screen.findByText("Your vault is locked")).toBeInTheDocument();
    unmount();
    expect(unsubscribe).toHaveBeenCalled();
  });

  it("shows an error when status fails", async () => {
    renderWithClient(<App />, fakeClient({ status: vi.fn().mockRejectedValue({ code: "internal", message: "boom" }) }));
    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
  });
});
