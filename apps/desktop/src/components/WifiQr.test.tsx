import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ClientProvider } from "../api/context";
import { DEFAULT_SETTINGS, SettingsProvider } from "../settings";
import { fakeClient } from "../test/fakeClient";
import { WifiQr } from "./WifiQr";

function setup(showWifiQr: boolean) {
  const client = fakeClient({
    settings: vi.fn().mockResolvedValue({ ...DEFAULT_SETTINGS, show_wifi_qr: showWifiQr }),
    wifiQr: vi.fn().mockResolvedValue("<svg/>"),
  });
  render(
    <ClientProvider client={client}>
      <SettingsProvider>
        <WifiQr itemId="r1" />
      </SettingsProvider>
    </ClientProvider>,
  );
  return { client, user: userEvent.setup() };
}

describe("WifiQr", () => {
  it("shows the code right away when the setting says so", async () => {
    const { client } = setup(true);
    expect(await screen.findByRole("img", { name: /QR code to join/ })).toHaveAttribute("src", expect.stringContaining("svg"));
    expect(client.wifiQr).toHaveBeenCalledWith("r1");
  });

  it("asks first otherwise", async () => {
    const { client, user } = setup(false);
    const button = await screen.findByRole("button", { name: "Show QR code" });
    expect(client.wifiQr).not.toHaveBeenCalled();
    await user.click(button);
    expect(await screen.findByRole("img")).toBeInTheDocument();
  });
});
