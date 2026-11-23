import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { Onboarding } from "./Onboarding";

function setup(overrides = {}) {
  const client = fakeClient({
    joinShowCode: vi.fn().mockResolvedValue({ code: "PASSWORD365-PAIR:1:NEW", qr_svg: "<svg xmlns='http://www.w3.org/2000/svg'/>" }),
    joinAcceptScan: vi.fn().mockReturnValue(new Promise(() => {})),
    cancelPairing: vi.fn().mockResolvedValue(undefined),
    joinWithCode: vi.fn().mockResolvedValue("123 456"),
    joinWait: vi.fn().mockResolvedValue(undefined),
    joinComplete: vi.fn().mockResolvedValue(null),
    ...overrides,
  });
  const onDone = vi.fn();
  renderWithClient(<Onboarding onDone={onDone} />, client);
  return { client, onDone, user: userEvent.setup() };
}

describe("Set up with another device", () => {
  it("pairs with the code, waits for approval and finishes with the master password", async () => {
    const { client, onDone, user } = setup();
    await user.click(screen.getByRole("button", { name: "Set up with another device" }));
    await user.click(await screen.findByRole("button", { name: "Or paste a code from the other device" }));
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    await user.type(screen.getByLabelText("Pairing code"), "  PASSWORD365-PAIR:1:ABC ");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(client.joinWithCode).toHaveBeenCalledWith("PASSWORD365-PAIR:1:ABC");
    expect(await screen.findByLabelText("Master password")).toBeInTheDocument();
    expect(client.joinWait).toHaveBeenCalled();
    await user.type(screen.getByLabelText("Master password"), "secret pw");
    await user.type(screen.getByLabelText("Name for this device"), "Work laptop");
    await user.click(screen.getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(client.joinComplete).toHaveBeenCalledWith("secret pw", "Work laptop");
  });

  it("waits for approval only once, however often the screen re-renders", async () => {
    const joinWait = vi.fn().mockReturnValue(new Promise(() => {}));
    const { user } = setup({ joinWait });
    await user.click(screen.getByRole("button", { name: "Set up with another device" }));
    await user.click(await screen.findByRole("button", { name: "Or paste a code from the other device" }));
    await user.type(screen.getByLabelText("Pairing code"), "X");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByLabelText("Verification code");
    await new Promise((r) => setTimeout(r, 50));
    expect(joinWait).toHaveBeenCalledTimes(1);
  });

  it("shows the code to compare while waiting for approval", async () => {
    const { user } = setup({ joinWait: vi.fn().mockReturnValue(new Promise(() => {})) });
    await user.click(screen.getByRole("button", { name: "Set up with another device" }));
    await user.click(await screen.findByRole("button", { name: "Or paste a code from the other device" }));
    await user.type(screen.getByLabelText("Pairing code"), "X");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByLabelText("Verification code")).toHaveTextContent("123 456");
    expect(screen.getByRole("status")).toHaveTextContent("Waiting for approval on the other device");
  });

  it("shows errors and keeps the user on the step", async () => {
    const { onDone, user } = setup({
      joinComplete: vi.fn().mockRejectedValue({ code: "wrong_credentials", message: "wrong master password" }),
      joinWithCode: vi.fn().mockResolvedValue("1"),
    });
    await user.click(screen.getByRole("button", { name: "Set up with another device" }));
    await user.click(await screen.findByRole("button", { name: "Or paste a code from the other device" }));
    await user.type(screen.getByLabelText("Pairing code"), "X");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await user.type(await screen.findByLabelText("Master password"), "nope");
    await user.click(screen.getByRole("button", { name: "Finish" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("wrong master password");
    expect(onDone).not.toHaveBeenCalled();
  });

  it("goes back and cancels the pairing", async () => {
    const { client, user } = setup();
    await user.click(screen.getByRole("button", { name: "Set up with another device" }));
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(client.cancelPairing).toHaveBeenCalled();
    expect(screen.getByRole("heading", { name: "Create your vault" })).toBeInTheDocument();
  });

  it("shows a QR code first; the device with the vault scans it, and the six digits appear here", async () => {
    let scanned: (digits: string) => void = () => {};
    const joinAcceptScan = vi.fn().mockReturnValue(new Promise<string>((resolve) => (scanned = resolve)));
    const { client, user } = setup({ joinAcceptScan, joinWait: vi.fn().mockReturnValue(new Promise(() => {})) });
    await user.click(screen.getByRole("button", { name: "Set up with another device" }));
    expect(await screen.findByAltText("QR code to join this vault")).toBeInTheDocument();
    expect(screen.getByText(/Waiting for the other device to scan/)).toBeInTheDocument();
    expect(client.joinShowCode).toHaveBeenCalledTimes(1);
    scanned("654 321");
    expect(await screen.findByLabelText("Verification code")).toHaveTextContent("654 321");
    expect(client.joinWait).toHaveBeenCalled();
  });

  it("goes back to pasting a code, and stops the code that was being shown", async () => {
    const { client, user } = setup();
    await user.click(screen.getByRole("button", { name: "Set up with another device" }));
    await screen.findByAltText("QR code to join this vault");
    await user.click(screen.getByRole("button", { name: "Or paste a code from the other device" }));
    expect(client.cancelPairing).toHaveBeenCalled();
    expect(screen.getByLabelText("Pairing code")).toBeInTheDocument();
  });
});

