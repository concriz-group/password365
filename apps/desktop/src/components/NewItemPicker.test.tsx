import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { categories } from "../test/fixtures";
import { NewItemPicker } from "./NewItemPicker";

describe("NewItemPicker", () => {
  it("shows the main categories as tiles and the rest as a list, and picks one", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    const { container } = render(<NewItemPicker categories={categories} onPick={onPick} onCancel={vi.fn()} />);
    const tiles = [...container.querySelectorAll(".category-tile")].map((t) => t.textContent);
    expect(tiles).toEqual(["Login", "Secure Note", "Credit Card", "Identity", "Password", "API Credential"]);
    expect(container.querySelectorAll(".category-tile svg")).toHaveLength(6);
    expect(container.querySelectorAll(".category-row")).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: /Credit Card/ }));
    expect(onPick).toHaveBeenCalledWith("card");
  });

  it("searches categories and cancels", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(<NewItemPicker categories={categories} onPick={vi.fn()} onCancel={onCancel} />);
    await user.type(screen.getByLabelText("Search for a category"), "router");
    expect(screen.getByRole("button", { name: /Wireless Router/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Login/ })).not.toBeInTheDocument();
    await user.clear(screen.getByLabelText("Search for a category"));
    await user.type(screen.getByLabelText("Search for a category"), "zzz");
    expect(screen.getByText(/No category matches/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalled();
  });
});
