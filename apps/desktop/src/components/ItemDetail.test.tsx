import { SettingsProvider } from "../settings";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { categories, item } from "../test/fixtures";
import { ItemDetail } from "./ItemDetail";
import { MASK } from "./SecretRow";
import { SEQUENCE_LABEL } from "../lib/autoType";

function setup(overrides = {}) {
  const client = fakeClient({
    item: vi.fn().mockResolvedValue(item),
    reveal: vi.fn().mockImplementation((_id: string, field: string) => Promise.resolve(`${field}-value`)),
    ...overrides,
  });
  const props = {
    id: "i1",
    vaultName: () => "Personal",
    onEdit: vi.fn(),
    onDeleted: vi.fn(),
    copy: vi.fn().mockResolvedValue(undefined),
  };
  renderWithClient(<ItemDetail {...props} />, client);
  return { client, props, user: userEvent.setup() };
}

describe("ItemDetail", () => {
  it("offers auto-type only with a password or an own sequence", async () => {
    const passkeyOnly = { ...item, has_password: false, fields: [] };
    setup({ item: vi.fn().mockResolvedValue(passkeyOnly) });
    expect(await screen.findByRole("heading", { name: "GitHub" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Auto-type" })).not.toBeInTheDocument();
  });

  it("keeps auto-type for an own sequence without a password", async () => {
    const own = { ...item, has_password: false, fields: [{ label: SEQUENCE_LABEL, value: "{USERNAME}{ENTER}", concealed: false, section: "", input: "text" }] };
    setup({ item: vi.fn().mockResolvedValue(own) });
    expect(await screen.findByRole("button", { name: "Auto-type" })).toBeInTheDocument();
  });

  it("shows the login's passkey and removes it after confirming", async () => {
    const withPasskey = { ...item, passkey: { rp_id: "github.com", user_name: "octo" } };
    const { client, user } = setup({ item: vi.fn().mockResolvedValueOnce(withPasskey).mockResolvedValue(item) });
    expect(await screen.findByText("github.com · octo")).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "Remove" })[0]!);
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(screen.getAllByRole("button", { name: "Remove" })[0]!);
    await user.click(screen.getByRole("button", { name: "Remove passkey" }));
    expect(client.removePasskey).toHaveBeenCalledWith("i1");
    await waitFor(() => expect(screen.queryByText("github.com · octo")).not.toBeInTheDocument());
  });

  it("shows fields with secrets masked", async () => {
    setup();
    expect(await screen.findByRole("heading", { name: "GitHub" })).toBeInTheDocument();
    expect(screen.getByText("Login · Personal")).toBeInTheDocument();
    expect(screen.getByLabelText("Password")).toHaveTextContent(MASK);
    expect(screen.getByLabelText("Notes")).toHaveTextContent(MASK);
    expect(screen.getByText("https://github.com")).toBeInTheDocument();
    expect(screen.getByText("dev")).toBeInTheDocument();
    expect(screen.getByText(/2 previous passwords/)).toBeInTheDocument();
  });

  it("reveals and hides a secret on demand", async () => {
    const { client, user } = setup();
    await screen.findByLabelText("Password");
    const [revealPassword] = screen.getAllByRole("button", { name: "Reveal" });
    await user.click(revealPassword!);
    expect(client.reveal).toHaveBeenCalledWith("i1", "password");
    expect(screen.getByLabelText("Password")).toHaveTextContent("password-value");
    await user.click(screen.getByRole("button", { name: "Hide" }));
    expect(screen.getByLabelText("Password")).toHaveTextContent(MASK);
  });

  it("copies secrets without displaying them, and copies the username", async () => {
    const { props, user } = setup();
    await screen.findByLabelText("Password");
    const copyButtons = screen.getAllByRole("button", { name: "Copy" });
    await user.click(copyButtons[0]!); // username
    expect(props.copy).toHaveBeenCalledWith("octo");
    await user.click(copyButtons[1]!); // password
    expect(props.copy).toHaveBeenLastCalledWith("password-value");
    expect(screen.getByLabelText("Password")).toHaveTextContent(MASK);
    expect(screen.getAllByRole("button", { name: "Copied" })).toHaveLength(1);
  });

  it("shows reveal errors", async () => {
    const { user } = setup({ reveal: vi.fn().mockRejectedValue({ code: "locked", message: "the vault is locked" }) });
    await screen.findByLabelText("Password");
    await user.click(screen.getAllByRole("button", { name: "Reveal" })[0]!);
    expect(await screen.findByRole("alert")).toHaveTextContent("locked");
  });

  it("shows copy errors", async () => {
    const { user } = setup({ reveal: vi.fn().mockRejectedValue({ code: "locked", message: "nope" }) });
    await screen.findByLabelText("Password");
    await user.click(screen.getAllByRole("button", { name: "Copy" })[1]!);
    expect(await screen.findByRole("alert")).toHaveTextContent("nope");
  });

  it("edits and deletes with confirmation", async () => {
    const { client, props, user } = setup();
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    expect(props.onEdit).toHaveBeenCalledWith(item);

    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Move to Recently Deleted" }));
    expect(client.deleteItem).toHaveBeenCalledWith("i1");
    expect(props.onDeleted).toHaveBeenCalled();
  });

  it("asks in a popup that names the item, starts on Cancel and closes with Escape or a click outside", async () => {
    const { client, user } = setup();
    await user.click(await screen.findByRole("button", { name: "Delete" }));
    const popup = screen.getByRole("alertdialog");
    expect(popup).toHaveTextContent("Move \"GitHub\" to Recently Deleted?");
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(popup.parentElement as HTMLElement);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(client.deleteItem).not.toHaveBeenCalled();
  });

  it("shows load and delete errors", async () => {
    setup({ item: vi.fn().mockRejectedValue({ code: "not_found", message: "item not found" }) });
    expect(await screen.findByRole("alert")).toHaveTextContent("item not found");
  });

  it("shows delete errors", async () => {
    const { user } = setup({ deleteItem: vi.fn().mockRejectedValue({ code: "io", message: "disk" }) });
    await user.click(await screen.findByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Move to Recently Deleted" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("disk");
  });

  it("omits optional sections", async () => {
    setup({
      item: vi.fn().mockResolvedValue({
        ...item,
        username: null,
        urls: [],
        tags: [],
        has_password: false,
        has_totp: false,
        has_notes: false,
      }),
    });
    await screen.findByRole("heading", { name: "GitHub" });
    expect(screen.queryByText("Username")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();
    expect(screen.queryByText("Tags")).not.toBeInTheDocument();
  });

  it("asks to review an item after a concurrent password change, then clears it", async () => {
    const onChanged = vi.fn();
    const client = fakeClient({ item: vi.fn().mockResolvedValue({ ...item, needs_review: true }) });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} copy={vi.fn()} onChanged={onChanged} />,
      client,
    );
    expect(await screen.findByRole("note")).toHaveTextContent("changed on two devices at the same time");
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("note")).not.toBeInTheDocument());
    expect(client.markReviewed).toHaveBeenCalledWith("i1");
    expect(onChanged).toHaveBeenCalled();
  });

  it("shows an error if the review cannot be saved", async () => {
    const client = fakeClient({
      item: vi.fn().mockResolvedValue({ ...item, needs_review: true }),
      markReviewed: vi.fn().mockRejectedValue({ code: "locked", message: "Vault is locked" }),
    });
    renderWithClient(<ItemDetail id="i1" vaultName={() => ""} onEdit={vi.fn()} onDeleted={vi.fn()} copy={vi.fn()} />, client);
    await userEvent.click(await screen.findByRole("button", { name: "Done" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Vault is locked");
  });
});

describe("ItemDetail favorites", () => {
  it("adds the item to Favorites and takes it out again", async () => {
    const onChanged = vi.fn();
    const client = fakeClient({ item: vi.fn().mockResolvedValue(item) });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} copy={vi.fn()} onChanged={onChanged} />,
      client,
    );
    const star = await screen.findByRole("button", { name: "Favorite" });
    expect(star).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(star);
    expect(client.setFavorite).toHaveBeenCalledWith("i1", true);
    await waitFor(() => expect(star).toHaveAttribute("aria-pressed", "true"));
    expect(onChanged).toHaveBeenCalled();
    await userEvent.click(star);
    expect(client.setFavorite).toHaveBeenLastCalledWith("i1", false);
  });

  it("shows an error when the favorite cannot be saved", async () => {
    const client = fakeClient({
      item: vi.fn().mockResolvedValue(item),
      setFavorite: vi.fn().mockRejectedValue({ code: "locked", message: "Vault is locked" }),
    });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} copy={vi.fn()} />,
      client,
    );
    await userEvent.click(await screen.findByRole("button", { name: "Favorite" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Vault is locked");
  });
});

describe("ItemDetail archive", () => {
  it("moves an item to the Archive", async () => {
    const onArchived = vi.fn();
    const client = fakeClient({ item: vi.fn().mockResolvedValue(item) });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} onArchived={onArchived} copy={vi.fn()} />,
      client,
    );
    await userEvent.click(await screen.findByRole("button", { name: "Archive" }));
    expect(client.setArchived).toHaveBeenCalledWith("i1", true);
    await waitFor(() => expect(onArchived).toHaveBeenCalled());
  });

  it("says an archived item is archived and restores it", async () => {
    const onArchived = vi.fn();
    const client = fakeClient({ item: vi.fn().mockResolvedValue({ ...item, archived: true }) });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} onArchived={onArchived} copy={vi.fn()} />,
      client,
    );
    expect(await screen.findByRole("note")).toHaveTextContent("in the Archive");
    expect(screen.queryByRole("button", { name: "Archive" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Restore" }));
    expect(client.setArchived).toHaveBeenCalledWith("i1", false);
    await waitFor(() => expect(onArchived).toHaveBeenCalled());
  });

  it("shows an error when archiving fails", async () => {
    const client = fakeClient({
      item: vi.fn().mockResolvedValue(item),
      setArchived: vi.fn().mockRejectedValue({ code: "locked", message: "Vault is locked" }),
    });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} copy={vi.fn()} />,
      client,
    );
    await userEvent.click(await screen.findByRole("button", { name: "Archive" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Vault is locked");
  });
});

describe("ItemDetail category fields", () => {
  it("shows plain fields and reveals concealed ones on demand", async () => {
    const card = {
      ...item,
      kind: "card",
      username: null,
      has_password: false,
      has_totp: false,
      has_notes: false,
      fields: [
        { label: "cardholder name", concealed: false, value: "Ana" , section: "", input: "text" as const },
        { label: "number", concealed: true, value: null , section: "", input: "text" as const },
      ],
    };
    const copy = vi.fn().mockResolvedValue(undefined);
    const client = fakeClient({
      item: vi.fn().mockResolvedValue(card),
      revealField: vi.fn().mockResolvedValue("4111"),
      categories: vi.fn().mockResolvedValue(categories),
    });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} copy={copy} />,
      client,
    );
    expect(await screen.findByText("Ana")).toBeInTheDocument();
    expect(await screen.findByText("Credit Card · Personal")).toBeInTheDocument();
    expect(screen.getByLabelText("number")).toHaveTextContent("••••");
    expect(client.revealField).not.toHaveBeenCalled();
    await userEvent.click(screen.getAllByRole("button", { name: "Reveal" })[0]!);
    expect(await screen.findByText("4111")).toBeInTheDocument();
    expect(client.revealField).toHaveBeenCalledWith("i1", "number");
    await userEvent.click(screen.getAllByRole("button", { name: "Copy" })[0]!);
    expect(copy).toHaveBeenCalledWith("Ana");
  });
});

describe("ItemDetail auto-type", () => {
  it("shows the sequence and windows and types into the previous window", async () => {
    const client = fakeClient({
      item: vi.fn().mockResolvedValue({
        ...item,
        fields: [
          { label: "auto-type sequence", concealed: false, value: "{PASSWORD}{ENTER}" , section: "", input: "text" as const },
          { label: "auto-type window", concealed: false, value: "*A*\n*B*" , section: "", input: "text" as const },
        ],
      }),
    });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} copy={vi.fn()} />,
      client,
    );
    expect(await screen.findByText(/Windows: \*A\*, \*B\*/)).toBeInTheDocument();
    expect(screen.queryByText("auto-type sequence")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Auto-type" }));
    expect(client.autoType).toHaveBeenCalledWith("i1");
  });

  it("shows auto-type errors", async () => {
    const client = fakeClient({
      item: vi.fn().mockResolvedValue(item),
      autoType: vi.fn().mockRejectedValue({ code: "io", message: "cannot type" }),
    });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} copy={vi.fn()} />,
      client,
    );
    expect(await screen.findByText("{USERNAME}{TAB}{PASSWORD}{ENTER}")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Auto-type" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("cannot type");
  });

  it("groups fields under their section and shows one-time password fields as codes", async () => {
    const totpCode = vi.fn().mockResolvedValue({ code: "123456", remaining: 20, period: 30 });
    const person = {
      ...item,
      kind: "identity",
      has_password: false,
      has_totp: false,
      has_notes: false,
      username: null,
      fields: [
        { label: "first name", concealed: false, value: "Ana", section: "Identification", input: "text" as const },
        { label: "bank 2fa", concealed: false, value: null, section: "Family", input: "totp" as const },
      ],
    };
    const client = fakeClient({ item: vi.fn().mockResolvedValue(person), totpCode });
    renderWithClient(
      <ItemDetail id="i1" vaultName={() => "Personal"} onEdit={vi.fn()} onDeleted={vi.fn()} copy={vi.fn()} />,
      client,
    );
    expect(await screen.findByRole("heading", { name: "Identification" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Family" })).toHaveTextContent("bank 2fa");
    await waitFor(() => expect(screen.getByLabelText("bank 2fa")).toHaveTextContent("123 456"));
    expect(totpCode).toHaveBeenCalledWith("i1", "bank 2fa");
  });
});

describe("ItemDetail in Portuguese", () => {
  it("translates the view, category fields and sections but keeps the user's own labels", async () => {
    const { ClientProvider } = await import("../api/context");
    const { I18nProvider } = await import("../i18n");
    const { render } = await import("@testing-library/react");
    const person = {
      ...item,
      kind: "identity",
      has_password: false,
      has_totp: false,
      has_notes: false,
      username: null,
      fields: [
        { label: "first name", concealed: false, value: "Ana", section: "Identification", input: "text" as const },
        { label: "pet", concealed: false, value: "Rex", section: "Family", input: "text" as const },
      ],
    };
    const client = fakeClient({
      item: vi.fn().mockResolvedValue(person),
      settings: vi.fn().mockResolvedValue({ auto_lock_minutes: 10, language: "pt-BR" }),
    });
    render(
      <ClientProvider client={client}>
        <SettingsProvider><I18nProvider>
          <ItemDetail id="i1" vaultName={() => "Pessoal"} onEdit={vi.fn()} onDeleted={vi.fn()} copy={vi.fn()} />
        </I18nProvider></SettingsProvider>
      </ClientProvider>,
    );
    expect(await screen.findByRole("heading", { name: "Identificação" })).toBeInTheDocument();
    expect(screen.getByText("nome")).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Family" })).toHaveTextContent("pet");
    expect(screen.getByRole("button", { name: "Editar" })).toBeInTheDocument();
    expect(screen.getByText(/Modificado em/)).toBeInTheDocument();
  });
});

describe("ItemDetail — item menu", () => {
  it("moves, duplicates, archives and deletes from the ⋮ menu", async () => {
    const vaults = [
      { id: "v1", name: "Personal", item_count: 1 },
      { id: "v2", name: "Work", item_count: 0 },
    ];
    const client = fakeClient({
      item: vi.fn().mockResolvedValue(item),
      moveItem: vi.fn().mockResolvedValue({ ...item, vault_id: "v2" }),
      duplicateItem: vi.fn().mockResolvedValue({ ...item, id: "copy" }),
    });
    const props = { onEdit: vi.fn(), onDeleted: vi.fn(), onArchived: vi.fn(), onChanged: vi.fn(), onOpen: vi.fn() };
    renderWithClient(
      <ItemDetail id="i1" vaultName={(v) => (v === "v2" ? "Work" : "Personal")} copy={vi.fn()} vaults={vaults} {...props} />,
      client,
    );
    const user = userEvent.setup();
    const more = await screen.findByRole("button", { name: "More actions" });
    await user.click(more);
    await user.click(screen.getByRole("menuitem", { name: "Move…" }));
    expect(screen.getByRole("menuitem", { name: "Work" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Personal" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("menuitem", { name: "Work" }));
    expect(client.moveItem).toHaveBeenCalledWith("i1", "v2");
    expect(await screen.findByText(/· Work/)).toBeInTheDocument();
    await user.click(more);
    await user.click(screen.getByRole("menuitem", { name: "Duplicate" }));
    expect(client.duplicateItem).toHaveBeenCalledWith("i1", "GitHub copy");
    expect(props.onOpen).toHaveBeenCalledWith("copy");
    await user.click(more);
    await user.click(screen.getByRole("menuitem", { name: "Add to Favorites" }));
    expect(client.setFavorite).toHaveBeenCalledWith("i1", true);
    await user.click(more);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    await user.click(more);
    await user.click(screen.getByRole("menuitem", { name: "Archive" }));
    expect(client.setArchived).toHaveBeenCalledWith("i1", true);
    await user.click(more);
    await user.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(screen.getByRole("button", { name: "Move to Recently Deleted" })).toBeInTheDocument();
  });

  it("reports move errors", async () => {
    const client = fakeClient({ item: vi.fn().mockResolvedValue(item), moveItem: vi.fn().mockRejectedValue({ code: "not_found", message: "vault not found" }), duplicateItem: vi.fn().mockRejectedValue({ code: "validation", message: "bad title" }) });
    renderWithClient(<ItemDetail id="i1" vaultName={() => "Personal"} copy={vi.fn()} onEdit={vi.fn()} onDeleted={vi.fn()} vaults={[{ id: "v2", name: "Work", item_count: 0 }]} />, client);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "More actions" }));
    await user.click(screen.getByRole("menuitem", { name: "Move…" }));
    await user.click(screen.getByRole("menuitem", { name: "Work" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("vault not found");
  });
});
