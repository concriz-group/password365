import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ClientProvider } from "../api/context";
import { DEFAULT_SETTINGS } from "../settings";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { DeveloperSettings } from "./DeveloperSettings";
import { SshApprovalPrompt } from "./SshApprovalPrompt";

const keys = {
  address: "/Users/me/.password365/agent.sock",
  keys: [
    { id: "a", title: "GitHub", fingerprint: "SHA256:abc", problem: null },
    { id: "b", title: "Old RSA", fingerprint: null, problem: "unreadable" },
    { id: "c", title: "Locked", fingerprint: null, problem: "encrypted" },
  ],
};

describe("DeveloperSettings", () => {
  it("turns the agent on and shows the ssh configuration and the keys", async () => {
    const set = vi.fn();
    const client = fakeClient({ sshKeys: vi.fn().mockResolvedValue(keys) });
    const { rerender } = render(
      <ClientProvider client={client}>
        <DeveloperSettings settings={DEFAULT_SETTINGS} disabled={false} set={set} />
      </ClientProvider>,
    );
    const user = userEvent.setup();
    expect(await screen.findByText("SHA256:abc")).toBeInTheDocument();
    expect(screen.getByText(/Not an OpenSSH Ed25519/)).toBeInTheDocument();
    expect(screen.getByText(/passphrase/, { selector: ".warn" })).toBeInTheDocument();
    expect(screen.queryByLabelText("SSH configuration")).not.toBeInTheDocument();
    await user.click(screen.getByLabelText("Use the SSH agent"));
    expect(set).toHaveBeenCalledWith({ ssh_agent: true });
    rerender(
      <ClientProvider client={client}>
        <DeveloperSettings settings={{ ...DEFAULT_SETTINGS, ssh_agent: true }} disabled={false} set={set} />
      </ClientProvider>,
    );
    expect(screen.getByLabelText("SSH configuration")).toHaveTextContent('IdentityAgent "/Users/me/.password365/agent.sock"');
    await user.click(screen.getByRole("button", { name: "Copy" }));
  });

  it("uses the pipe as is on Windows and says when there are no keys", async () => {
    const client = fakeClient({ sshKeys: vi.fn().mockResolvedValue({ address: "\\\\.\\pipe\\password365-ssh-agent", keys: [] }) });
    renderWithClient(<DeveloperSettings settings={{ ...DEFAULT_SETTINGS, ssh_agent: true }} disabled={false} set={vi.fn()} />, client);
    expect(await screen.findByText(/No SSH Key items yet/)).toBeInTheDocument();
    expect(screen.getByLabelText("SSH configuration")).toHaveTextContent("IdentityAgent \\\\.\\pipe\\password365-ssh-agent");
  });
});

describe("SshApprovalPrompt", () => {
  it("answers allow or deny", async () => {
    const client = fakeClient();
    const onDone = vi.fn();
    renderWithClient(<SshApprovalPrompt request={{ id: 7, title: "GitHub", fingerprint: "SHA256:abc" }} onDone={onDone} />, client);
    expect(screen.getByRole("alertdialog", { name: "Allow SSH key?" })).toHaveTextContent("GitHub");
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Allow" }));
    expect(client.sshAnswer).toHaveBeenCalledWith(7, true);
    await vi.waitFor(() => expect(onDone).toHaveBeenCalled());
    await user.click(screen.getByRole("button", { name: "Deny" }));
    expect(client.sshAnswer).toHaveBeenLastCalledWith(7, false);
  });

  it("names the program that asks", () => {
    renderWithClient(
      <SshApprovalPrompt request={{ id: 8, title: "GitHub", fingerprint: "SHA256:abc", program: "git" }} onDone={vi.fn()} />,
      fakeClient(),
    );
    expect(screen.getByRole("alertdialog")).toHaveTextContent("“git” wants to use the SSH key “GitHub”");
  });
});
