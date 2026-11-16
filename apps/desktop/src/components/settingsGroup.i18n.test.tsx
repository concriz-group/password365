import { SettingsProvider } from "../settings";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";

import type { Client } from "../api/client";
import { ClientProvider } from "../api/context";
import { I18nProvider } from "../i18n";
import { fakeClient } from "../test/fakeClient";
import { AddDevice } from "./AddDevice";
import { DevicesPanel, syncFileName } from "./DevicesPanel";
import { ErrorMessage } from "./ErrorMessage";
import { SettingsPanel } from "./SettingsPanel";
import { SyncReminder } from "./SyncReminder";
import { ExportSection } from "./ExportSection";
import { Transfer } from "./Transfer";

const files = {
  openCsv: vi.fn(),
  openImport: vi.fn(),
  openAnyFile: vi.fn(),
  saveFile: vi.fn(),
  saveCsv: vi.fn(),
  openBackup: vi.fn(),
  saveBackup: vi.fn(),
  openSyncFile: vi.fn(),
  saveSyncFile: vi.fn(),
};

function inLanguage(language: string, ui: ReactElement, overrides = {}) {
  let saved = { auto_lock_minutes: 10, language };
  const client: Client = fakeClient({
    settings: vi.fn(async () => saved),
    updateSettings: vi.fn(async (next: typeof saved) => {
      saved = next;
    }),
    backupInfo: vi.fn().mockResolvedValue({ last_backup_ms: null, due: true }),
    thisDevice: vi.fn().mockResolvedValue({ id: "me", name: "Notebook" }),
    ...overrides,
  });
  render(
    <ClientProvider client={client}>
      <SettingsProvider><I18nProvider>{ui}</I18nProvider></SettingsProvider>
    </ClientProvider>,
  );
  return { client, user: userEvent.setup() };
}

describe("Settings in another language", () => {
  it("switches the interface when a language is chosen and saves it with the other settings", async () => {
    const { client, user } = inLanguage("", <SettingsPanel onClose={vi.fn()} files={files} />);
    await user.click(screen.getByRole("button", { name: "Advanced" }));
    const select = await screen.findByLabelText("Language");
    expect(screen.getByRole("option", { name: "Use system default" })).toHaveValue("");
    expect(screen.getByRole("option", { name: "Português (Brasil)" })).toHaveAttribute("lang", "pt-BR");
    await user.selectOptions(select, "pt-BR");
    expect(await screen.findByRole("heading", { name: "Configurações" })).toBeInTheDocument();
    expect(client.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ auto_lock_minutes: 10, language: "pt-BR" }));
    expect(screen.getByLabelText("Idioma")).toHaveValue("pt-BR");
    await user.click(screen.getByRole("button", { name: "Segurança" }));
    expect(screen.getByRole("option", { name: "2 horas" })).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Bloquear após ficar inativo por"), "5");
    expect(client.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ auto_lock_minutes: 5, language: "pt-BR" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Salvo.");
    await user.click(screen.getByRole("button", { name: "Backup" }));
    expect(screen.getByText("Último backup:")).toBeInTheDocument();
    expect(screen.getByText("nunca")).toBeInTheDocument();
  });

  it("shows Spanish when saved", async () => {
    const { user } = inLanguage("es", <SettingsPanel onClose={vi.fn()} files={files} />);
    expect(await screen.findByRole("heading", { name: "Ajustes" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Seguridad" }));
    expect(screen.getByRole("button", { name: "Imprimir Emergency Kit" })).toBeInTheDocument();
  });
});

describe("errors", () => {
  it("are translated by code, keeping the backend's detail where the text has room", async () => {
    inLanguage(
      "pt-BR",
      <>
        <ErrorMessage error={{ code: "wrong_credentials", message: "wrong master password" }} />
        <ErrorMessage error={{ code: "validation", message: "title is empty" }} />
        <ErrorMessage error={{ code: "something_new", message: "raw message" }} />
      </>,
    );
    await waitFor(() =>
      expect(screen.getAllByRole("alert").map((a) => a.textContent)).toEqual([
        "Senha mestra ou Secret Key incorreta.",
        "Confira o que foi digitado: title is empty",
        "raw message",
      ]),
    );
  });
});

describe("devices and transfer in Portuguese", () => {
  const phone = { id: "d1", name: "Celular", paired_at: 1, last_sync: null, pending_changes: 1, addr: null, out_of_sync: true };

  it("uses plural forms and translated device texts", async () => {
    const { user } = inLanguage("pt-BR", <DevicesPanel vaults={[]} onSynced={vi.fn()} onClose={vi.fn()} files={files} />, {
      devices: vi.fn().mockResolvedValue([phone]),
      syncDevice: vi.fn().mockResolvedValue({ changed: 1, conflicts: ["x", "y"] }),
    });
    expect(await screen.findByText("Última sincronização: nunca · 1 alteração daqui ainda não chegou nele")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sincronizar agora" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Sincronizado com Celular: 1 alteração recebida. 2 itens tinham duas senhas diferentes",
    );
    expect(screen.getByRole("button", { name: "importe um arquivo de sincronização" })).toBeInTheDocument();
    expect(syncFileName("??")).toBe("Password365 sync for device.qlsync");
  });

  it("translates the first-sync banner", async () => {
    inLanguage("pt-BR", <SyncReminder onOpenDevices={vi.fn()} />, { devices: vi.fn().mockResolvedValue([phone]) });
    expect(await screen.findByRole("status")).toHaveTextContent("Sincronizando com Celular pela primeira vez…");
  });

  it("translates the sync reminder and the add-device steps", async () => {
    inLanguage(
      "pt-BR",
      <>
        <SyncReminder onOpenDevices={vi.fn()} />
        <AddDevice vaults={[]} onAdded={vi.fn()} onClose={vi.fn()} />
      </>,
      {
        devices: vi.fn().mockResolvedValue([
          { ...phone, last_sync: 1 },
          { ...phone, id: "d2", name: "Tablet", last_sync: 1 },
        ]),
        inviteDevice: vi.fn(() => new Promise(() => undefined)),
      },
    );
    expect(await screen.findByRole("note")).toHaveTextContent(
      "Falta 1 alteração feita aqui em Celular (e mais 1 dispositivo).",
    );
    expect(screen.getByRole("heading", { name: "Adicionar um dispositivo" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancelar" })).toBeInTheDocument();
    expect(screen.getByText("Preparando…")).toBeInTheDocument();
  });

  it("translates the migration picker", async () => {
    inLanguage("pt-BR", <Transfer vaults={[]} files={files} onImported={vi.fn()} onClose={vi.fn()} />);
    expect(await screen.findByRole("heading", { name: "Migrar seus dados" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Um gerenciador de senhas" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Planilha" })).toBeInTheDocument();
  });

  it("translates the export warning", async () => {
    inLanguage("pt-BR", <ExportSection vaults={[]} files={files} />);
    expect(await screen.findByRole("heading", { name: "Exportar" })).toBeInTheDocument();
    expect(screen.getByText("texto simples").tagName).toBe("STRONG");
  });
});
