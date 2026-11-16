import { classify, fillOwnFields, fillProfile, findProfileFields, fitted, normalize } from "./profileFields";

function page(html: string): Document {
  document.body.innerHTML = html;
  return document;
}

const names = (fields: { name: string }[]) => fields.map((f) => f.name);

describe("profile fields", () => {
  it("reads autocomplete names, ignoring sections and billing/shipping words", () => {
    const doc = page(`
      <form>
        <input autocomplete="section-a shipping given-name">
        <input autocomplete="family-name">
        <input autocomplete="billing postal-code">
        <select autocomplete="country"><option value="BR">Brazil</option></select>
        <input autocomplete="username" name="email">
      </form>`);
    expect(names(findProfileFields(doc).identity)).toEqual(["given-name", "family-name", "postal-code", "country"]);
  });

  it("guesses from names and labels in several languages", () => {
    const doc = page(`
      <form>
        <label for="n">Número do cartão</label><input id="n">
        <input name="cvv">
        <input placeholder="MM/AA" name="validade">
        <input name="nome_titular">
        <input name="cep"><input name="cidade"><input type="tel" name="x"><input type="email" name="y">
        <input name="nothing">
      </form>`);
    const found = findProfileFields(doc);
    expect(names(found.card)).toEqual(["cc-number", "cc-csc", "cc-exp", "cc-name"]);
    expect(names(found.identity)).toEqual(["postal-code", "address-level2", "tel", "email"]);
  });

  it("leaves login forms, lone e-mail boxes and card forms without a number alone", () => {
    const login = page(`<form><input type="email"><input name="first_name"><input type="password"></form>`);
    expect(findProfileFields(login)).toEqual({ identity: [], card: [] });
    const newsletter = page(`<form><input type="email"><input name="cvv"></form>`);
    expect(findProfileFields(newsletter)).toEqual({ identity: [], card: [] });
  });

  it("skips disabled, read-only and other kinds of inputs", () => {
    page(`<input name="cep" disabled><input name="cep" readonly><input type="checkbox" name="cep"><input name="cep" autocomplete="one-time-code">`);
    for (const input of document.querySelectorAll("input")) expect(classify(input)).toBeNull();
  });

  it("fills inputs and selects, shaping the expiry date the way the page wants", () => {
    const doc = page(`
      <form>
        <input autocomplete="cc-number">
        <input autocomplete="cc-exp" placeholder="MM/YY">
        <input autocomplete="cc-exp" maxlength="7">
        <input autocomplete="cc-exp" type="month">
        <select autocomplete="cc-exp-month"><option value="">--</option><option value="4">April</option></select>
        <select autocomplete="cc-exp-year"><option>2028</option><option>2029</option></select>
        <input autocomplete="cc-csc">
      </form>`);
    const fields = findProfileFields(doc).card;
    const filled = fillProfile(fields, {
      "cc-number": "4111111111114242",
      "cc-exp": "04/29",
      "cc-exp-month": "04",
      "cc-exp-year": "2029",
    });
    expect(filled).toBe(6);
    const values = fields.map((f) => f.element.value);
    expect(values).toEqual(["4111111111114242", "04/29", "04/2029", "2029-04", "4", "2029", ""]);
  });

  it("fills an identity's address and country", () => {
    const doc = page(`
      <form>
        <input autocomplete="street-address">
        <input autocomplete="address-line2">
        <select autocomplete="country"><option value="BR">Brazil</option><option value="PT">Portugal</option></select>
        <select autocomplete="address-level1"><option>SP</option></select>
      </form>`);
    const fields = findProfileFields(doc).identity;
    expect(
      fillProfile(fields, { "street-address": "Rua A\\nApto 3", "address-line1": "Rua A", "address-line2": "Apto 3", "country-name": "Portugal", "address-level1": "RJ" }),
    ).toBe(3);
    expect(fields.map((f) => f.element.value)).toEqual(["Rua A", "Apto 3", "PT", "SP"]);
    const change = vi.fn();
    fields[2]!.element.addEventListener("change", change);
    fillProfile([fields[2]!], { country: "BR" });
    expect(change).toHaveBeenCalled();
  });
});

describe("country and own fields", () => {
  it("recognises Brazilian documents and the house number from the country file", () => {
    const doc = page(`
      <form>
        <label>CPF <input name="doc"></label>
        <label>Número <input id="num" maxlength="6"></label>
        <label>Número do cartão <input id="cc"></label>
        <label>Logradouro <input id="rua"></label>
        <label>Bairro <input id="bairro"></label>
      </form>`);
    const { identity, card } = findProfileFields(doc);
    expect(identity.map((f) => f.name)).toEqual(["cpf", "house-number", "address-line1", "address-level3"]);
    expect(card.map((f) => f.name)).toEqual(["cc-number"]);
    const values = { cpf: "123.456.789-09", "house-number": "100", "address-line1": "Rua A", "street-address": "Rua A, 100", "address-level3": "Centro" };
    expect(fillProfile(identity, values)).toBe(4);
    expect(identity.map((f) => f.element.value)).toEqual(["123.456.789-09", "100", "Rua A", "Centro"]);
  });

  it("puts the whole street in one box when the form has no number box", () => {
    const doc = page(`<form><label>Endereço <input id="a"></label><label>Cidade <input id="c"></label></form>`);
    const { identity } = findProfileFields(doc);
    fillProfile(identity, { "address-line1": "Rua A", "street-address": "Rua A, 100", "address-level2": "X" });
    expect(identity[0]!.element.value).toBe("Rua A, 100");
  });

  it("types only the digits when a formatted value does not fit", () => {
    const input = document.createElement("input");
    input.maxLength = 11;
    expect(fitted(input, "123.456.789-09")).toBe("12345678909");
    input.maxLength = 3;
    expect(fitted(input, "123.456.789-09")).toBe("123.456.789-09");
    expect(fitted(document.createElement("input"), "abc")).toBe("abc");
  });

  it("fills boxes named like the item's own fields", () => {
    const doc = page(`
      <form>
        <label>Matricula <input id="m"></label>
        <label>Nome <input id="n"></label>
        <input aria-label="Plano">
      </form>
      <form><input type="password"><label>Matrícula <input id="hidden"></label></form>`);
    const values = { "label:matrícula": "A-77", "label:plano": "Gold", "label:": "x", "given-name": "Ana" };
    expect(fillOwnFields(doc, values, [])).toBe(2);
    expect((doc.getElementById("m") as HTMLInputElement).value).toBe("A-77");
    expect((doc.getElementById("hidden") as HTMLInputElement).value).toBe("");
    expect(fillOwnFields(doc, { email: "x" }, [])).toBe(0);
    expect(normalize("  Nº  Matrícula! ")).toBe("nº matricula");
  });
});
