import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { Unlock } from "./Unlock";

describe("Unlock", () => {
  it("unlocks with the master password", async () => {
    const user = userEvent.setup();
    const unlock = vi.fn().mockResolvedValue(undefined);
    const onUnlocked = vi.fn();
    renderWithClient(<Unlock onUnlocked={onUnlocked} />, fakeClient({ unlock }));

    const submit = screen.getByRole("button", { name: "Unlock" });
    expect(submit).toBeDisabled();
    await user.type(screen.getByLabelText("Master password"), "secret password");
    await user.click(submit);

    expect(unlock).toHaveBeenCalledWith("secret password", undefined);
    expect(onUnlocked).toHaveBeenCalled();
  });

  it("shows wrong-password errors without asking for the Secret Key", async () => {
    const user = userEvent.setup();
    const unlock = vi.fn().mockRejectedValue({ code: "wrong_credentials", message: "wrong master password or Secret Key" });
    renderWithClient(<Unlock onUnlocked={vi.fn()} />, fakeClient({ unlock }));
    await user.type(screen.getByLabelText("Master password"), "nope");
    await user.click(screen.getByRole("button", { name: "Unlock" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("wrong master password");
    expect(screen.queryByLabelText("Secret Key")).not.toBeInTheDocument();
    expect(screen.getByText(/still opens with the previous password/)).toBeInTheDocument();
  });

  it("asks for the Secret Key when the device does not have it", async () => {
    const user = userEvent.setup();
    const unlock = vi
      .fn()
      .mockRejectedValueOnce({ code: "validation", message: "Secret Key required on this device" })
      .mockResolvedValueOnce(undefined);
    const onUnlocked = vi.fn();
    renderWithClient(<Unlock onUnlocked={onUnlocked} />, fakeClient({ unlock }));

    await user.type(screen.getByLabelText("Master password"), "pw");
    await user.click(screen.getByRole("button", { name: "Unlock" }));
    await user.type(await screen.findByLabelText("Secret Key"), "P365-KEY");
    await user.click(screen.getByRole("button", { name: "Unlock" }));

    expect(unlock).toHaveBeenLastCalledWith("pw", "P365-KEY");
    expect(onUnlocked).toHaveBeenCalledTimes(1);
  });

  it("lets the user open the Secret Key field manually", async () => {
    const user = userEvent.setup();
    renderWithClient(<Unlock onUnlocked={vi.fn()} />, fakeClient());
    await user.click(screen.getByRole("button", { name: /New device/ }));
    expect(screen.getByLabelText("Secret Key")).toBeInTheDocument();
  });
});

describe("Unlock — wrong passwords", () => {
  it("counts down after too many wrong passwords and asks for the Secret Key when it was forgotten", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const unlock = vi
      .fn()
      .mockRejectedValueOnce({ code: "too_many_attempts", message: "too many wrong passwords — try again in 2 s" })
      .mockRejectedValueOnce({ code: "secret_key_required", message: "enter your Secret Key" })
      .mockResolvedValue(undefined);
    const client = fakeClient({ unlock });
    const onUnlocked = vi.fn();
    renderWithClient(<Unlock onUnlocked={onUnlocked} />, client);
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    await user.type(screen.getByLabelText("Master password"), "guess");
    await user.click(screen.getByRole("button", { name: "Unlock" }));
    expect(await screen.findByText("Too many wrong passwords. Try again in 2 seconds.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Unlock" })).toBeDisabled();
    for (const _ of [1, 2]) {
      await act(async () => {
        vi.advanceTimersByTime(1000);
      });
    }
    expect(screen.getByRole("button", { name: "Unlock" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "Unlock" }));
    expect(await screen.findByText(/does not have the Secret Key/)).toBeInTheDocument();
    await user.type(screen.getByLabelText("Secret Key"), "P365-ABC");
    await user.click(screen.getByRole("button", { name: "Unlock" }));
    expect(unlock).toHaveBeenLastCalledWith("guess", "P365-ABC");
    vi.useRealTimers();
  });
});
