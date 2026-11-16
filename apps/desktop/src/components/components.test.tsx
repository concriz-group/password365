import { render, renderHook, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { ErrorMessage } from "./ErrorMessage";
import { PasswordInput } from "./PasswordInput";

describe("PasswordInput", () => {
  it("toggles visibility and reports changes", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<PasswordInput label="Password" value="abc" onChange={onChange} />);
    const input = screen.getByLabelText("Password");
    expect(input).toHaveAttribute("type", "password");
    await user.click(screen.getByRole("button", { name: "Show Password" }));
    expect(input).toHaveAttribute("type", "text");
    await user.click(screen.getByRole("button", { name: "Hide Password" }));
    expect(input).toHaveAttribute("type", "password");
    await user.type(input, "d");
    expect(onChange).toHaveBeenCalledWith("abcd");
  });
});

describe("ErrorMessage", () => {
  it("renders nothing without an error", () => {
    const { container } = render(<ErrorMessage error={null} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the message", () => {
    render(<ErrorMessage error={{ code: "locked", message: "the vault is locked" }} />);
    expect(screen.getByRole("alert")).toHaveTextContent("the vault is locked");
  });
});

describe("useAction", () => {
  it("tracks busy, result and error", async () => {
    const action = vi.fn<() => Promise<number>>().mockResolvedValueOnce(42).mockRejectedValueOnce({ code: "io", message: "x" });
    const { result } = renderHook(() => useAction(action));

    let value: number | undefined;
    await act(async () => {
      value = await result.current.run();
    });
    expect(value).toBe(42);
    expect(result.current.error).toBeNull();

    await act(async () => {
      value = await result.current.run();
    });
    expect(value).toBeUndefined();
    expect(result.current.error?.code).toBe("io");
    expect(result.current.busy).toBe(false);

    act(() => result.current.clearError());
    expect(result.current.error).toBeNull();
  });
});

describe("useClient", () => {
  it("throws outside a provider", () => {
    expect(() => renderHook(() => useClient())).toThrow(/ClientProvider/);
  });
});
