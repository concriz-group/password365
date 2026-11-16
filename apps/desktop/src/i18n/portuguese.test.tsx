import { SettingsProvider } from "../settings";
import { screen } from "@testing-library/react";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { Sidebar } from "../components/Sidebar";
import { NewItemPicker } from "../components/NewItemPicker";
import { translateCategories } from "../hooks/useCategories";
import { relativeTime } from "../lib/relativeTime";
import { categories } from "../test/fixtures";
import { ALL } from "../lib/scope";
import { I18nProvider, catalogs } from ".";
import { translator } from "../../../../i18n/translate";

const portuguese = () =>
  fakeClient({ settings: vi.fn().mockResolvedValue({ auto_lock_minutes: 10, language: "pt-BR" }) });

describe("the interface in Portuguese", () => {
  it("shows the sidebar in the saved language", async () => {
    renderWithClient(
      <SettingsProvider><I18nProvider>
        <Sidebar vaults={[]} selected={ALL} onSelect={vi.fn()} onCreate={vi.fn()} onShowDeleted={vi.fn()} />
      </I18nProvider></SettingsProvider>,
      portuguese(),
    );
    expect(await screen.findByRole("button", { name: /Todos os itens/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Excluídos recentemente" })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("pt-BR");
  });

  it("translates the new-item screen and category names", async () => {
    const t = translator(catalogs, "pt-BR");
    renderWithClient(
      <SettingsProvider><I18nProvider>
        <NewItemPicker categories={translateCategories(categories, t)} onPick={vi.fn()} onCancel={vi.fn()} />
      </I18nProvider></SettingsProvider>,
      portuguese(),
    );
    expect(await screen.findByRole("heading", { name: "O que você gostaria de adicionar?" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Cartão de crédito/ })).toBeInTheDocument();
    const unknown = translateCategories([{ kind: "spaceship", label: "Spaceship", plural: "Spaceships", fields: [] }], t);
    expect(unknown[0]).toMatchObject({ label: "Spaceship", plural: "Spaceships" });
  });

  it("says how long ago in Portuguese", () => {
    const t = translator(catalogs, "pt-BR");
    expect(relativeTime(0, 2 * 3_600_000, t)).toBe("há 2 horas");
    expect(relativeTime(null, 0, t)).toBe("nunca");
  });
});
