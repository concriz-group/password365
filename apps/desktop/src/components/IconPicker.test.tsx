import { fireEvent, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { IconPicker } from "./IconPicker";

const PNG = "data:image/png;base64,iVBORw0KGgo=";

function setup(icon: string | null, convert = vi.fn().mockResolvedValue(PNG)) {
  const onChange = vi.fn();
  renderWithClient(<IconPicker title="Bank" icon={icon} onChange={onChange} convert={convert} />, fakeClient());
  return { onChange, convert, user: userEvent.setup() };
}

describe("IconPicker", () => {
  it("chooses a new icon from a picture", async () => {
    const { onChange, convert, user } = setup(null);
    expect(screen.getByText("B")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Change the icon" }));
    expect(screen.queryByRole("menuitem", { name: "Remove icon" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("menuitem", { name: "Choose a picture…" }));
    const picture = new File(["x"], "logo.png", { type: "image/png" });
    await user.upload(screen.getByLabelText("Choose a picture…"), picture);
    expect(convert).toHaveBeenCalledWith(picture);
    await vi.waitFor(() => expect(onChange).toHaveBeenCalledWith(PNG));
    fireEvent.change(screen.getByLabelText("Choose a picture…"), { target: { files: [] } });
  });

  it("removes the icon and reports pictures it cannot read", async () => {
    const { onChange, user } = setup(PNG, vi.fn().mockRejectedValue(new Error("bad")));
    expect(document.querySelector("img.avatar")).toHaveAttribute("src", PNG);
    await user.click(screen.getByRole("button", { name: "Change the icon" }));
    await user.click(screen.getByRole("menuitem", { name: "Remove icon" }));
    expect(onChange).toHaveBeenCalledWith(null);
    await user.upload(screen.getByLabelText("Choose a picture…"), new File(["x"], "x.png", { type: "image/png" }));
    expect(await screen.findByText(/could not be opened/)).toBeInTheDocument();
  });
});

describe("IconPicker — gallery", () => {
  it("picks a gallery icon in a chosen colour, and shows the category's icon meanwhile", async () => {
    const onChange = vi.fn();
    renderWithClient(<IconPicker title="Bank" kind="bank_account" icon={null} onChange={onChange} />, fakeClient());
    expect(document.querySelector(".avatar.glyph")).toHaveStyle({ background: "#0d9488" });
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Change the icon" }));
    await user.click(screen.getByRole("menuitem", { name: "Choose from gallery…" }));
    const gallery = screen.getByRole("dialog", { name: "Icon gallery" });
    await user.click(screen.getByRole("button", { name: "#dc2626" }));
    expect(screen.getByRole("button", { name: "#dc2626" })).toHaveAttribute("aria-pressed", "true");
    await user.click(screen.getByRole("button", { name: "Home" }));
    expect(onChange).toHaveBeenCalledWith("builtin:home:#dc2626");
    expect(gallery).not.toBeInTheDocument();
  });

  it("shows a stored gallery icon and ignores unknown ones", () => {
    renderWithClient(<IconPicker title="Bank" icon="builtin:home:#16a34a" onChange={vi.fn()} />, fakeClient());
    expect(document.querySelector(".avatar.glyph")).toHaveStyle({ background: "#16a34a" });
  });
});

