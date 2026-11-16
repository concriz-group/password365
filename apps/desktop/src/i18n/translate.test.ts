import { catalogsFromFiles, pickLanguage, placeholders, translator } from "../../../../i18n/translate";
import { catalogs, languages } from ".";

describe("translate", () => {
  const files = {
    "/x/locales/en/meta.json": { name: "English" },
    "/x/locales/en/items.json": { title: "Items", count_one: "{count} item", count_other: "{count} items", hi: "Hi {name}" },
    "/x/locales/pt-BR/meta.json": { name: "Português (Brasil)" },
    "/x/locales/pt-BR/items.json": { title: "Itens", count_one: "{count} item", count_other: "{count} itens", n: 3 },
    "/x/locales/fr/items.json": {},
    "/x/elsewhere.json": { title: "ignored" },
  };

  it("builds catalogs and lists languages by their own name", () => {
    const { catalogs, languages } = catalogsFromFiles(files);
    expect(catalogs.en!["items.title"]).toBe("Items");
    expect(catalogs["pt-BR"]!["items.n"]).toBeUndefined();
    expect(languages.map((l) => l.name)).toEqual(["English", "fr", "Português (Brasil)"]);
  });

  it("falls back to English, then to the key, and fills placeholders and plurals", () => {
    const { catalogs } = catalogsFromFiles(files);
    const t = translator(catalogs, "pt-BR");
    expect(t("items.title")).toBe("Itens");
    expect(t("items.hi", { name: "Ana" })).toBe("Hi Ana");
    expect(t("items.count", { count: 1 })).toBe("1 item");
    expect(t("items.count", { count: 2 })).toBe("2 itens");
    expect(t("items.missing")).toBe("items.missing");
    expect(t("items.hi", { other: 1 })).toBe("Hi {name}");
    expect(translator(catalogs, "xx")("items.title")).toBe("Items");
  });

  it("picks an exact language, then the same base language, else English", () => {
    const available = ["en", "pt-BR", "es"];
    expect(pickLanguage(["pt-br"], available)).toBe("pt-BR");
    expect(pickLanguage(["pt-PT", "es"], available)).toBe("pt-BR");
    expect(pickLanguage(["de", "es-MX"], available)).toBe("es");
    expect(pickLanguage(["de"], available)).toBe("en");
    expect(placeholders("{b} and {a}")).toEqual(["a", "b"]);
  });
});

describe("the shipped translations", () => {
  const english = catalogs.en!;

  it("include English, Portuguese and Spanish", () => {
    expect(languages.map((l) => l.tag).sort()).toEqual(["en", "es", "pt-BR"]);
  });

  for (const { tag } of languages.filter((l) => l.tag !== "en")) {
    it(`${tag} has every English key with the same placeholders, and nothing else`, () => {
      const catalog = catalogs[tag]!;
      expect(Object.keys(catalog).filter((k) => !(k in english))).toEqual([]);
      const plain = (k: string) => k.replace(/_(zero|one|two|few|many|other)$/, "");
      const missing = Object.keys(english).filter((k) => !(k in catalog) && !(plain(k) !== k && `${plain(k)}_other` in catalog));
      expect(missing).toEqual([]);
      for (const [key, text] of Object.entries(catalog)) {
        const source = english[key] ?? english[`${plain(key)}_other`] ?? "";
        expect([key, placeholders(text)]).toEqual([key, placeholders(source)]);
      }
    });
  }
});

describe("translateOr", () => {
  it("uses the fallback only when the key is unknown", async () => {
    const { translateOr } = await import("../../../../i18n/translate");
    const t = translator(catalogs, "pt-BR");
    expect(translateOr(t, "common.save", "x")).toBe("Salvar");
    expect(translateOr(t, "fields.my own", "my own")).toBe("my own");
  });
});
