import { screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DEFAULT_PASSPHRASE, DEFAULT_PASSWORD, DEFAULT_PIN } from "../lib/generatorPresets";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { Generator } from "./Generator";

function setup(generate = vi.fn().mockResolvedValue({ value: "abc", bits: 130.4, strength: "excellent" })) {
  const client = fakeClient({ generate });
  const props = { copy: vi.fn().mockResolvedValue(undefined), onClose: vi.fn() };
  renderWithClient(<Generator {...props} />, client);
  return { generate, props, user: userEvent.setup() };
}

describe("Generator", () => {
  it("generates a default password on open and shows strength", async () => {
    const { generate } = setup();
    expect(await screen.findByLabelText("Generated value")).toHaveTextContent("abc");
    expect(screen.getByText("~130 bits · excellent")).toBeInTheDocument();
    expect(generate).toHaveBeenCalledWith(DEFAULT_PASSWORD);
  });

  it("switches modes", async () => {
    const { generate, user } = setup();
    await user.click(screen.getByRole("radio", { name: "Passphrase" }));
    expect(generate).toHaveBeenLastCalledWith(DEFAULT_PASSPHRASE);
    await user.click(screen.getByRole("radio", { name: "PIN" }));
    expect(generate).toHaveBeenLastCalledWith(DEFAULT_PIN);
  });

  it("adjusts password options", async () => {
    const { generate, user } = setup();
    fireEvent.change(screen.getByLabelText(/Length/), { target: { value: "32" } });
    expect(generate).toHaveBeenLastCalledWith(expect.objectContaining({ length: 32 }));
    await user.click(screen.getByLabelText("Symbols"));
    expect(generate).toHaveBeenLastCalledWith(
      expect.objectContaining({ classes: ["Lowercase", "Uppercase", "Digits"] }),
    );
    await user.click(screen.getByLabelText("Symbols"));
    expect(generate).toHaveBeenLastCalledWith(
      expect.objectContaining({ classes: ["Lowercase", "Uppercase", "Digits", "Symbols"] }),
    );
    await user.click(screen.getByLabelText("Avoid look-alike characters"));
    expect(generate).toHaveBeenLastCalledWith(expect.objectContaining({ exclude_ambiguous: true }));
  });

  it("adjusts passphrase and PIN options", async () => {
    const { generate, user } = setup();
    await user.click(screen.getByRole("radio", { name: "Passphrase" }));
    fireEvent.change(screen.getByLabelText(/Words/), { target: { value: "7" } });
    expect(generate).toHaveBeenLastCalledWith(expect.objectContaining({ words: 7 }));
    await user.click(screen.getByLabelText("Include a number"));
    expect(generate).toHaveBeenLastCalledWith(expect.objectContaining({ include_number: false }));
    await user.click(screen.getByRole("radio", { name: "PIN" }));
    fireEvent.change(screen.getByLabelText(/Digits/), { target: { value: "8" } });
    expect(generate).toHaveBeenLastCalledWith({ type: "pin", digits: 8 });
  });

  it("regenerates, copies and closes", async () => {
    const { generate, props, user } = setup();
    await screen.findByText("~130 bits · excellent");
    await user.click(screen.getByRole("button", { name: "Regenerate" }));
    expect(generate).toHaveBeenCalledTimes(2);
    await user.click(screen.getByRole("button", { name: "Copy" }));
    expect(props.copy).toHaveBeenCalledWith("abc");
    expect(screen.getByRole("button", { name: "Copied" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(props.onClose).toHaveBeenCalled();
  });

  it("shows errors", async () => {
    setup(vi.fn().mockRejectedValue({ code: "validation", message: "invalid recipe" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("invalid recipe");
  });
});
