import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ShortcutInput } from "./ShortcutInput";

describe("ShortcutInput", () => {
  it("records a shortcut, ignores lone modifiers and clears it", async () => {
    const onChange = vi.fn();
    render(<ShortcutInput id="s" label="Lock Password365" value="Shift+Super+KeyL" onChange={onChange} mac />);
    const input = screen.getByLabelText("Lock Password365");
    expect(input).toHaveValue("⇧⌘L");
    const user = userEvent.setup();
    await user.click(input);
    expect(input).toHaveAttribute("placeholder", "Press the keys…");
    await user.keyboard("{Shift}");
    expect(onChange).not.toHaveBeenCalled();
    await user.keyboard("{Control>}{Alt>}k{/Alt}{/Control}");
    expect(onChange).toHaveBeenCalledWith("Ctrl+Alt+KeyK");
    await user.click(input);
    await user.keyboard("{Escape}");
    expect(input).not.toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Clear the shortcut for Lock Password365" }));
    expect(onChange).toHaveBeenLastCalledWith("");
  });
});
