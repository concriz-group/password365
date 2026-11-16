import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { Onboarding } from "./Onboarding";

const PW = "correct horse battery";

/** The button unlocks once the core's strength estimate for the typed password arrives. */
async function clickWhenEnabled(user: ReturnType<typeof userEvent.setup>, name: string) {
  const button = screen.getByRole("button", { name });
  await waitFor(() => expect(button).toBeEnabled());
  await user.click(button);
}

describe("Onboarding", () => {
  it("disables submit until the password is long enough and confirmed", async () => {
    const user = userEvent.setup();
    renderWithClient(<Onboarding onDone={vi.fn()} />, fakeClient());
    const submit = screen.getByRole("button", { name: "Create account" });
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("Master password"), "short");
    await user.type(screen.getByLabelText("Repeat master password"), "short");
    expect(submit).toBeDisabled();

    await user.clear(screen.getByLabelText("Master password"));
    await user.type(screen.getByLabelText("Master password"), PW);
    expect(screen.getByText("Passwords do not match.")).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Repeat master password"));
    await user.type(screen.getByLabelText("Repeat master password"), PW);
    await waitFor(() => expect(submit).toBeEnabled());
  });

  it("shows the Secret Key and requires confirmation before continuing", async () => {
    const user = userEvent.setup();
    const onDone = vi.fn();
    const createAccount = vi.fn().mockResolvedValue("Q1-AAAA-BBBB");
    renderWithClient(<Onboarding onDone={onDone} />, fakeClient({ createAccount }));

    await user.type(screen.getByLabelText("Master password"), PW);
    await user.type(screen.getByLabelText("Repeat master password"), PW);
    await clickWhenEnabled(user, "Create account");

    expect(createAccount).toHaveBeenCalledWith(PW, PW);
    expect(await screen.findByLabelText("Secret Key")).toHaveTextContent("Q1-AAAA-BBBB");
    const open = screen.getByRole("button", { name: "Open my vault" });
    expect(open).toBeDisabled();
    await user.click(screen.getByLabelText("I have saved my Secret Key"));
    await user.click(open);
    expect(onDone).toHaveBeenCalled();
  });

  it("prints the Emergency Kit with the Secret Key but never the password", async () => {
    const user = userEvent.setup();
    const printPage = vi.fn().mockResolvedValue(undefined);
    const createAccount = vi.fn().mockResolvedValue("Q1-KIT-KEY");
    renderWithClient(<Onboarding onDone={vi.fn()} />, fakeClient({ createAccount, printPage }));
    await user.type(screen.getByLabelText("Master password"), PW);
    await user.type(screen.getByLabelText("Repeat master password"), PW);
    await clickWhenEnabled(user, "Create account");
    await user.click(await screen.findByRole("button", { name: "Print Emergency Kit" }));
    expect(printPage).toHaveBeenCalled();
    const kit = screen.getByLabelText("Emergency Kit");
    expect(kit).toHaveTextContent("Q1-KIT-KEY");
    expect(kit).not.toHaveTextContent(PW);
  });

  it("shows print errors", async () => {
    const user = userEvent.setup();
    const printPage = vi.fn().mockRejectedValue({ code: "internal", message: "no printer" });
    renderWithClient(
      <Onboarding onDone={vi.fn()} />,
      fakeClient({ createAccount: vi.fn().mockResolvedValue("Q1-X"), printPage }),
    );
    await user.type(screen.getByLabelText("Master password"), PW);
    await user.type(screen.getByLabelText("Repeat master password"), PW);
    await clickWhenEnabled(user, "Create account");
    await user.click(await screen.findByRole("button", { name: "Print Emergency Kit" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("no printer");
  });

  it("shows backend errors", async () => {
    const user = userEvent.setup();
    const createAccount = vi.fn().mockRejectedValue({ code: "io", message: "disk full" });
    renderWithClient(<Onboarding onDone={vi.fn()} />, fakeClient({ createAccount }));
    await user.type(screen.getByLabelText("Master password"), PW);
    await user.type(screen.getByLabelText("Repeat master password"), PW);
    await clickWhenEnabled(user, "Create account");
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
  });

  it("shows a strength meter that explains what is missing", async () => {
    const user = userEvent.setup();
    renderWithClient(<Onboarding onDone={vi.fn()} />, fakeClient());
    expect(screen.getByText(/Use at least 10 characters/)).toBeInTheDocument();
    await user.type(screen.getByLabelText("Master password"), "abcdefg");
    expect(await screen.findByText(/3 more characters needed/)).toBeInTheDocument();
    await user.type(screen.getByLabelText("Master password"), "hij");
    expect(await screen.findByText(/Too easy to guess/)).toBeInTheDocument();
    expect(screen.getByRole("meter", { name: "Password strength" })).toHaveAttribute("aria-valuenow", "2");
    await user.type(screen.getByLabelText("Master password"), "klmnop");
    expect(await screen.findByText(/Strong enough/)).toBeInTheDocument();
  });

  it("keeps the button disabled when the strength check fails", async () => {
    const user = userEvent.setup();
    const passwordStrength = vi.fn().mockRejectedValue({ code: "internal", message: "boom" });
    renderWithClient(<Onboarding onDone={vi.fn()} />, fakeClient({ passwordStrength }));
    await user.type(screen.getByLabelText("Master password"), PW);
    await user.type(screen.getByLabelText("Repeat master password"), PW);
    await waitFor(() => expect(passwordStrength).toHaveBeenCalled());
    expect(screen.getByRole("button", { name: "Create account" })).toBeDisabled();
  });
});
