import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ClientProvider } from "../api/context";
import { fakeClient } from "../test/fakeClient";
import { TotpRow, formatCode } from "./TotpRow";

describe("TotpRow", () => {
  afterEach(() => vi.useRealTimers());

  it("splits codes in two halves", () => {
    expect(formatCode("123456")).toBe("123 456");
    expect(formatCode("1234567")).toBe("1234 567");
  });

  it("shows the code, counts down and fetches the next one when it expires", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const totpCode = vi
      .fn()
      .mockResolvedValueOnce({ code: "123456", remaining: 2, period: 30 })
      .mockResolvedValueOnce({ code: "654321", remaining: 30, period: 30 });
    const client = fakeClient({ totpCode });
    render(
      <ClientProvider client={client}>
        <TotpRow itemId="i1" copy={vi.fn()} />
      </ClientProvider>,
    );
    expect(await screen.findByLabelText("One-time password")).toHaveTextContent("123 456");
    expect(screen.getByLabelText("Seconds left")).toHaveTextContent("2s");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(screen.getByLabelText("Seconds left")).toHaveTextContent("1s");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });
    expect(screen.getByLabelText("One-time password")).toHaveTextContent("654 321");
    expect(totpCode).toHaveBeenCalledTimes(2);
  });

  it("copies the code without the space", async () => {
    const copy = vi.fn().mockResolvedValue(undefined);
    const client = fakeClient({ totpCode: vi.fn().mockResolvedValue({ code: "123456", remaining: 20, period: 30 }) });
    render(
      <ClientProvider client={client}>
        <TotpRow itemId="i1" copy={copy} />
      </ClientProvider>,
    );
    await screen.findByText(/123 456/);
    await userEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(copy).toHaveBeenCalledWith("123456");
    expect(screen.getByRole("button", { name: "Copied" })).toBeInTheDocument();
  });

  it("explains when no code can be made from the stored secret", async () => {
    const client = fakeClient({
      totpCode: vi.fn().mockRejectedValue({ code: "validation", message: "invalid one-time password setup" }),
    });
    render(
      <ClientProvider client={client}>
        <TotpRow itemId="i1" copy={vi.fn()} />
      </ClientProvider>,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("invalid one-time password setup");
    expect(screen.getByRole("button", { name: "Copy" })).toBeDisabled();
  });
});
