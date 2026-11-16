import { SettingsProvider } from "../settings";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DEFAULT_PASSWORD } from "../lib/generatorPresets";
import { fakeClient, renderWithClient } from "../test/fakeClient";
import { categories, item, personal, work } from "../test/fixtures";
import { ItemEditor } from "./ItemEditor";

function setup(editing: boolean, overrides = {}, extra: { kind?: string; item?: typeof item } = {}) {
  const client = fakeClient({
    categories: vi.fn().mockResolvedValue(categories),
    createItem: vi.fn().mockResolvedValue(item),
    updateItem: vi.fn().mockResolvedValue(item),
    reveal: vi.fn().mockResolvedValue("old notes"),
    generate: vi.fn().mockResolvedValue({ value: "GENERATED-PW", bits: 130, strength: "excellent" }),
    ...overrides,
  });
  const props = {
    item: editing ? (extra.item ?? item) : null,
    kind: extra.kind,
    vaults: [personal, work],
    defaultVaultId: "v2",
    onSaved: vi.fn(),
    onCancel: vi.fn(),
  };
  renderWithClient(<ItemEditor {...props} />, client);
  return { client, props, user: userEvent.setup() };
}

describe("ItemEditor — create", () => {
  it("creates a login in the default vault", async () => {
    const { client, props, user } = setup(false);
    expect(screen.getByLabelText("Vault")).toHaveValue("v2");
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();

    await user.type(screen.getByLabelText("Title"), "Mail");
    await user.type(screen.getByLabelText("Username"), "me");
    await user.type(screen.getByLabelText("Password"), "pw");
    await user.type(screen.getByLabelText("One-time password"), "JBSWY3DPEHPK3PXP");
    await user.type(screen.getByLabelText(/Websites/), "https://mail.com");
    await user.type(screen.getByLabelText("Notes"), "n");
    await user.type(screen.getByLabelText(/Tags/), "a, b");
    await user.click(save);

    expect(client.createItem).toHaveBeenCalledWith({
      vault_id: "v2",
      kind: "login",
      title: "Mail",
      username: "me",
      urls: ["https://mail.com"],
      tags: ["a", "b"],
      favorite: false,
      password: "pw",
      totp: "JBSWY3DPEHPK3PXP",
      notes: "n",
      region: "",
    });
    expect(props.onSaved).toHaveBeenCalledWith(item);
  });

  it("fills a generated password", async () => {
    const { client, user } = setup(false);
    await user.click(screen.getByRole("button", { name: "Generate strong password" }));
    expect(client.generate).toHaveBeenCalledWith(DEFAULT_PASSWORD);
    await waitFor(() => expect(screen.getByLabelText("Password")).toHaveValue("GENERATED-PW"));
  });

  it("hides login fields for secure notes", async () => {
    const { user } = setup(false, {}, { kind: "note" });
    expect(screen.queryByLabelText("Username")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Vault"), "v1");
    expect(screen.getByLabelText("Vault")).toHaveValue("v1");
  });

  it("shows save errors and cancels", async () => {
    const { props, user } = setup(false, {
      createItem: vi.fn().mockRejectedValue({ code: "validation", message: "title must not be empty" }),
    });
    await user.type(screen.getByLabelText("Title"), "x");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("title must not be empty");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(props.onCancel).toHaveBeenCalled();
  });
});

describe("ItemEditor — edit", () => {
  it("prefills fields, loads notes and keeps the password when left empty", async () => {
    const { client, user } = setup(true);
    expect(screen.getByLabelText("Title")).toHaveValue("GitHub");
    expect(screen.queryByLabelText("Type")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByLabelText("Notes")).toHaveValue("old notes"));
    expect(client.reveal).toHaveBeenCalledWith("i1", "notes");

    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.updateItem).toHaveBeenCalledWith(
      "i1",
      3,
      expect.objectContaining({ kind: undefined, password: undefined, notes: "old notes", tags: ["dev"] }),
    );
  });

  it("sends a new password when typed", async () => {
    const { client, user } = setup(true);
    await user.type(screen.getByLabelText(/New password/, { selector: "input" }), "fresh");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.updateItem).toHaveBeenCalledWith("i1", 3, expect.objectContaining({ password: "fresh" }));
  });
});

describe("ItemEditor — categories", () => {
  it("shows a password item without username or websites", () => {
    setup(false, {}, { kind: "password" });
    expect(screen.getByLabelText("Password")).toBeInTheDocument();
    expect(screen.queryByLabelText("Username")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/Websites/)).not.toBeInTheDocument();
  });

  it("creates a credit card from its template", async () => {
    const { client, user } = setup(false, {}, { kind: "card" });
    expect(await screen.findByRole("heading", { name: "New Credit Card" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Username")).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("Title"), "Visa");
    await user.type(screen.getByLabelText("Cardholder name"), "Ana");
    await user.type(screen.getByLabelText("Number"), "4111");
    expect(screen.getByLabelText("Number")).toHaveAttribute("type", "password");
    expect(screen.getByLabelText("Expiry date")).toHaveAttribute("type", "month");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.createItem).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: "card",
        fields: [
          { label: "cardholder name", value: "Ana", section: "", input: "text", concealed: false },
          { label: "number", value: "4111", section: "", input: "text", concealed: true },
          { label: "expiry date", value: "", section: "", input: "month_year", concealed: false },
        ],
      }),
    );
  });

  it("loads concealed values when editing and keeps extra fields", async () => {
    const card = {
      ...item,
      kind: "card",
      has_notes: false,
      fields: [
        { label: "number", concealed: true, value: null , section: "", input: "text" as const },
        { label: "nickname", concealed: false, value: "gold" , section: "", input: "text" as const },
      ],
    };
    const { client, user } = setup(true, { revealField: vi.fn().mockResolvedValue("4111") }, { item: card });
    await waitFor(() => expect(screen.getByLabelText("Number")).toHaveValue("4111"));
    expect(client.revealField).toHaveBeenCalledWith("i1", "number");
    expect(screen.getByLabelText("Nickname")).toHaveValue("gold");
    expect(screen.getByLabelText("Name of nickname")).toHaveValue("nickname");
    await user.clear(screen.getByLabelText("Nickname"));
    await user.type(screen.getByLabelText("Nickname"), "black");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.updateItem).toHaveBeenCalledWith(
      "i1",
      3,
      expect.objectContaining({
        fields: [
          expect.objectContaining({ label: "cardholder name", value: "" }),
          expect.objectContaining({ label: "number", value: "4111" }),
          expect.objectContaining({ label: "expiry date", value: "" }),
          expect.objectContaining({ label: "nickname", value: "black" }),
        ],
      }),
    );
  });

  it("adds typed fields and sections like 1Password", async () => {
    const { client, user } = setup(false, {}, { kind: "card" });
    await user.type(await screen.findByLabelText("Title"), "Visa");
    await user.click(screen.getByRole("button", { name: "Add another field" }));
    expect(screen.getAllByRole("menuitem").map((m) => m.textContent)).toContain("One-Time Password");
    await user.click(screen.getByRole("menuitem", { name: "Phone" }));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Phone")).toHaveAttribute("type", "tel");
    await user.type(screen.getByLabelText("Phone"), "555");
    await user.click(screen.getByRole("button", { name: "+ add more" }));
    await user.clear(screen.getByLabelText("Section name"));
    await user.type(screen.getByLabelText("Section name"), "Bank");
    await user.click(screen.getByRole("button", { name: "Add another field to Bank" }));
    await user.click(screen.getByRole("menuitem", { name: "Password" }));
    await user.clear(screen.getByLabelText("Name of password"));
    await user.type(screen.getByLabelText("Name of field"), "app PIN");
    await user.type(screen.getByLabelText("App PIN"), "1234");
    expect(screen.getByLabelText("App PIN")).toHaveAttribute("type", "password");
    await user.click(screen.getByRole("button", { name: "Add another field to Bank" }));
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Add another field to Bank" }));
    await user.click(screen.getByRole("menuitem", { name: "Text" }));
    await user.click(screen.getByRole("button", { name: "Remove text" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    const saved = vi.mocked(client.createItem).mock.calls[0]![0].fields!;
    expect(saved).toContainEqual({ label: "phone", value: "555", section: "", input: "phone", concealed: false });
    expect(saved).toContainEqual({ label: "app PIN", value: "1234", section: "Bank", input: "text", concealed: true });
    expect(saved.some((f) => f.label === "text")).toBe(false);
  });

  it("refuses two fields with the same name", async () => {
    const { client, user } = setup(false, {}, { kind: "card" });
    await user.type(await screen.findByLabelText("Title"), "Visa");
    await user.type(screen.getByLabelText("Cardholder name"), "Ana");
    await user.click(screen.getByRole("button", { name: "Add another field" }));
    await user.click(screen.getByRole("menuitem", { name: "Text" }));
    await user.clear(screen.getByLabelText("Name of text"));
    await user.type(screen.getByLabelText("Name of field"), "Cardholder Name");
    await user.type(screen.getAllByLabelText(/^Cardholder name$/i)[1]!, "x");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText(/already a field called "Cardholder Name"/)).toBeInTheDocument();
    expect(client.createItem).not.toHaveBeenCalled();
  });

  it("uses a text area for multi-line fields", async () => {
    setup(false, {}, { kind: "identity" });
    expect((await screen.findByLabelText("Address")).tagName).toBe("TEXTAREA");
  });
});

describe("ItemEditor — auto-type", () => {
  it("saves a sequence and window patterns as the item's auto-type fields", async () => {
    const { client, user } = setup(false, {}, { kind: "login" });
    await user.type(screen.getByLabelText("Title"), "VPN");
    await user.click(screen.getByText("Auto-type"));
    await user.click(screen.getByLabelText("Sequence"));
    await user.paste("{USERNAME}{TAB}{PASSWORD}{ENTER}{DELAY 900}{TOTP}");
    await user.type(screen.getByLabelText(/Windows/), "*VPN*");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.createItem).toHaveBeenCalledWith(
      expect.objectContaining({
        fields: [
          { label: "auto-type sequence", value: "{USERNAME}{TAB}{PASSWORD}{ENTER}{DELAY 900}{TOTP}" },
          { label: "auto-type window", value: "*VPN*" },
        ],
      }),
    );
  });

  it("loads existing auto-type settings and clears them", async () => {
    const withAutoType = {
      ...item,
      fields: [
        { label: "auto-type sequence", concealed: false, value: "{PASSWORD}{ENTER}" , section: "", input: "text" as const },
        { label: "auto-type window", concealed: false, value: "*Putty*" , section: "", input: "text" as const },
      ],
    };
    const { client, user } = setup(true, {}, { item: withAutoType });
    expect(screen.getByLabelText("Sequence")).toHaveValue("{PASSWORD}{ENTER}");
    expect(screen.queryByLabelText("Auto-type sequence")).not.toBeInTheDocument();
    await user.clear(screen.getByLabelText("Sequence"));
    await user.clear(screen.getByLabelText(/Windows/));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.updateItem).toHaveBeenCalledWith("i1", 3, expect.objectContaining({ fields: [] }));
  });
});

describe("ItemEditor in Spanish", () => {
  it("shows the field-type menu translated and names new fields in Spanish", async () => {
    const { ClientProvider } = await import("../api/context");
    const { I18nProvider } = await import("../i18n");
    const { render } = await import("@testing-library/react");
    const { categories } = await import("../test/fixtures");
    const client = fakeClient({
      categories: vi.fn().mockResolvedValue(categories),
      createItem: vi.fn().mockResolvedValue(item),
      settings: vi.fn().mockResolvedValue({ auto_lock_minutes: 10, language: "es" }),
    });
    const user = userEvent.setup();
    render(
      <ClientProvider client={client}>
        <SettingsProvider><I18nProvider>
          <ItemEditor item={null} kind="card" vaults={[]} defaultVaultId={null} onSaved={vi.fn()} onCancel={vi.fn()} />
        </I18nProvider></SettingsProvider>
      </ClientProvider>,
    );
    await user.click(await screen.findByRole("button", { name: "Añadir otro campo" }));
    await user.click(screen.getByRole("menuitem", { name: "Teléfono" }));
    expect(screen.getByLabelText("Nombre de teléfono")).toHaveValue("teléfono");
    expect(screen.getByRole("button", { name: "Guardar" })).toBeInTheDocument();
  });
});

describe("ItemEditor — countries", () => {
  const identity = {
    kind: "identity",
    label: "Identity",
    plural: "Identities",
    fields: [
      { label: "first name", input: "text" as const, concealed: false, section: "Identification" },
      { label: "address", input: "multiline" as const, concealed: false, section: "Address" },
    ],
    regions: {
      BR: [
        { label: "first name", input: "text" as const, concealed: false, section: "Identification" },
        { label: "CPF", input: "text" as const, concealed: false, section: "Identification", mask: "000.000.000-00", validate: "cpf" },
        { label: "address", name: "street", input: "text" as const, concealed: false, section: "Address" },
        { label: "state", input: "text" as const, concealed: false, section: "Address", options: ["RJ", "SP"] },
      ],
    },
  };

  it("uses the country's fields, masks and checks them, and keeps values when switching", async () => {
    const { client, user } = setup(false, { categories: vi.fn().mockResolvedValue([identity]) }, { kind: "identity" });
    const country = await screen.findByLabelText("Country or region");
    await waitFor(() => expect(screen.getByLabelText("First name")).toBeInTheDocument());
    expect(country).toHaveValue("");
    await user.type(screen.getByLabelText("Title"), "Me");
    await user.type(screen.getByLabelText("First name"), "Ana");
    await user.type(screen.getByRole("textbox", { name: "Address" }), "Rua A");
    await user.selectOptions(country, "BR");
    expect(screen.getByLabelText("First name")).toHaveValue("Ana");
    expect(screen.getByLabelText("Street")).toHaveValue("Rua A");
    await user.type(screen.getByLabelText("CPF"), "52998224724");
    expect(screen.getByLabelText("CPF")).toHaveValue("529.982.247-24");
    expect(screen.getByText(/does not look right/)).toBeInTheDocument();
    await user.type(screen.getByLabelText("CPF"), "{Backspace}5");
    expect(screen.queryByText(/does not look right/)).not.toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("State"), "SP");
    await user.click(screen.getByRole("button", { name: "Save" }));
    const input = vi.mocked(client.createItem).mock.calls[0]![0];
    expect(input.region).toBe("BR");
    expect(input.fields).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ label: "CPF", value: "529.982.247-25" }),
        expect.objectContaining({ label: "state", value: "SP" }),
        expect.objectContaining({ label: "address", value: "Rua A" }),
      ]),
    );
  });

  it("opens an item in its own country and defaults new ones from the language", async () => {
    const own = { ...item, kind: "identity", has_notes: false, region: "BR", fields: [{ label: "CPF", value: "1", concealed: false, section: "Identification", input: "text" as const }] };
    setup(true, { categories: vi.fn().mockResolvedValue([identity]) }, { item: own });
    // The select can show before the rows are laid out on the item's country.
    expect(await screen.findByLabelText("CPF")).toHaveValue("1");
    expect(screen.getByLabelText("Country or region")).toHaveValue("BR");
  });
});


describe("ItemEditor — icon", () => {
  it("sends a removed icon as empty and leaves an untouched one out", async () => {
    const withIcon = { ...item, icon: "data:image/png;base64,AA==" };
    const { client, user } = setup(true, {}, { item: withIcon });
    await user.click(await screen.findByRole("button", { name: "Change the icon" }));
    await user.click(screen.getByRole("menuitem", { name: "Remove icon" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(vi.mocked(client.updateItem).mock.calls[0]![2].icon).toBe("");
  });
});

describe("ItemEditor — country from the settings", () => {
  it("uses the chosen country for a new identity even with the app in English", async () => {
    const { SettingsProvider, DEFAULT_SETTINGS } = await import("../settings");
    const { ClientProvider } = await import("../api/context");
    const { render } = await import("@testing-library/react");
    const identity = {
      kind: "identity",
      label: "Identity",
      plural: "Identities",
      fields: [{ label: "first name", input: "text" as const, concealed: false, section: "" }],
      regions: { BR: [{ label: "CPF", input: "text" as const, concealed: false, section: "" }] },
    };
    const client = fakeClient({
      categories: vi.fn().mockResolvedValue([identity]),
      settings: vi.fn().mockResolvedValue({ ...DEFAULT_SETTINGS, region: "BR" }),
    });
    render(
      <ClientProvider client={client}>
        <SettingsProvider>
          <ItemEditor item={null} kind="identity" vaults={[personal]} defaultVaultId="v1" onSaved={vi.fn()} onCancel={vi.fn()} />
        </SettingsProvider>
      </ClientProvider>,
    );
    expect(await screen.findByLabelText("CPF")).toBeInTheDocument();
    expect(screen.getByLabelText("Country or region")).toHaveValue("BR");
  });
});


describe("ItemEditor — reordering", () => {
  it("moves fields with the handle's arrow keys and by dragging", async () => {
    const { client, user } = setup(false, {}, { kind: "card" });
    await user.type(await screen.findByLabelText("Title"), "Visa");
    await user.type(screen.getByLabelText("Cardholder name"), "Ana");
    await user.type(screen.getByLabelText("Number"), "4111");
    screen.getByRole("button", { name: "Move Number" }).focus();
    await user.keyboard("{ArrowUp}");
    const { fireEvent } = await import("@testing-library/react");
    const data = new Map<string, string>();
    const dataTransfer = { setData: (k: string, v: string) => data.set(k, v), getData: (k: string) => data.get(k) ?? "" };
    fireEvent.dragStart(screen.getByRole("button", { name: "Move Expiry date" }), { dataTransfer });
    fireEvent.drop(screen.getByRole("button", { name: "Move Number" }).parentElement!, { dataTransfer });
    await user.click(screen.getByRole("button", { name: "Save" }));
    const saved = vi.mocked(client.createItem).mock.calls[0]![0].fields!.map((f) => f.label);
    expect(saved.slice(0, 3)).toEqual(["expiry date", "number", "cardholder name"]);
  });
});

describe("ItemEditor — sign in with", () => {
  it("points a field at another login", async () => {
    const google = { ...item, id: "g1", title: "Google", kind: "login" };
    const { client, user } = setup(false, { items: vi.fn().mockResolvedValue([google, { ...google, id: "n1", title: "Note", kind: "note" }]) });
    await user.type(await screen.findByLabelText("Title"), "Figma");
    await user.click(screen.getByRole("button", { name: "Add another field" }));
    await user.click(screen.getByRole("menuitem", { name: "Sign in with" }));
    const select = await screen.findByRole("combobox", { name: "Sign in with" });
    expect(screen.queryByRole("option", { name: "Note" })).not.toBeInTheDocument();
    await user.selectOptions(select, "g1");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(vi.mocked(client.createItem).mock.calls[0]![0].fields).toContainEqual(
      expect.objectContaining({ label: "sign in with", value: "g1", input: "sign_in_with" }),
    );
  });
});

describe("ItemEditor — documents", () => {
  it("picks the file of a new document and attaches it after saving", async () => {
    const { fakeFiles } = await import("../test/fakeFiles");
    const files = fakeFiles();
    const client = fakeClient({ categories: vi.fn().mockResolvedValue(categories), createItem: vi.fn().mockResolvedValue({ ...item, id: "d1" }) });
    const onSaved = vi.fn();
    renderWithClient(<ItemEditor item={null} kind="document" vaults={[personal]} defaultVaultId="v1" onSaved={onSaved} onCancel={vi.fn()} files={files} />, client);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Choose the file…" }));
    expect(screen.getByText("File: passport.pdf")).toBeInTheDocument();
    expect(screen.getByLabelText("Title")).toHaveValue("passport.pdf");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.createItem).toHaveBeenCalledWith(expect.objectContaining({ kind: "document", title: "passport.pdf" }));
    expect(client.attachFile).toHaveBeenCalledWith("d1", "/tmp/passport.pdf");
    expect(onSaved).toHaveBeenCalled();
  });
});
