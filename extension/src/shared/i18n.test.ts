import { catalogs, language, t, tOr } from "./i18n";
import { helpFor } from "./help";

describe("extension texts", () => {
  it("come from the shared catalogs, with fallbacks", () => {
    expect(Object.keys(catalogs)).toEqual(expect.arrayContaining(["en", "pt-BR", "es"]));
    expect(language).toBe("en"); // jsdom's navigator says en-US
    expect(t("save")).toBe("Save");
    expect(tOr("nothing", "fallback")).toBe("fallback");
    expect(helpFor({ code: "weird", message: "raw" })).toBe("raw");
    expect(helpFor({ code: "host_missing", message: "raw" })).toContain("install-host");
    expect(catalogs["pt-BR"]!["extension.notNow"]).toBe("Agora não");
  });
});
