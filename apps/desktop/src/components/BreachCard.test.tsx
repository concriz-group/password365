import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ClientProvider } from "../api/context";
import { DEFAULT_SETTINGS, SettingsProvider } from "../settings";
import { fakeClient } from "../test/fakeClient";
import { BreachCard } from "./BreachCard";
import { PrivacySettings } from "./PrivacySettings";

function setup(on: boolean, found: unknown = []) {
  const client = fakeClient({
    settings: vi.fn().mockResolvedValue({ ...DEFAULT_SETTINGS, check_breached_passwords: on }),
    breachedPasswords: Array.isArray(found) ? vi.fn().mockResolvedValue(found) : vi.fn().mockRejectedValue(found),
  });
  const onOpenItem = vi.fn();
  render(
    <ClientProvider client={client}>
      <SettingsProvider>
        <BreachCard onOpenItem={onOpenItem} />
      </SettingsProvider>
    </ClientProvider>,
  );
  return { client, onOpenItem, user: userEvent.setup() };
}

describe("BreachCard", () => {
  it("says how to turn it on and asks nothing while off", () => {
    const { client } = setup(false);
    expect(screen.getByText(/Turn it on in Settings › Privacy/)).toBeInTheDocument();
    expect(client.breachedPasswords).not.toHaveBeenCalled();
  });

  it("checks on request and lists what was found", async () => {
    const { onOpenItem, user } = setup(true, [{ item: { id: "i1", title: "Mail", kind: "login" }, count: 3 }]);
    await user.click(await screen.findByRole("button", { name: "Check now" }));
    await user.click(await screen.findByRole("button", { name: /Mail/ }));
    expect(screen.getByText("seen 3 times in breaches")).toBeInTheDocument();
    expect(onOpenItem).toHaveBeenCalledWith("i1");
  });

  it("says when nothing was found, and shows errors", async () => {
    const { user } = setup(true);
    await user.click(await screen.findByRole("button", { name: "Check now" }));
    expect(await screen.findByText(/None of your passwords/)).toBeInTheDocument();
  });

  it("shows network errors", async () => {
    const { user } = setup(true, { code: "io", message: "offline" });
    await user.click(await screen.findByRole("button", { name: "Check now" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("offline");
  });
});

describe("PrivacySettings", () => {
  it("turns the check on", async () => {
    const set = vi.fn();
    render(<PrivacySettings settings={DEFAULT_SETTINGS} disabled={false} set={set} />);
    await userEvent.setup().click(screen.getByLabelText("Check for vulnerable passwords"));
    expect(set).toHaveBeenCalledWith({ check_breached_passwords: true });
  });
});
