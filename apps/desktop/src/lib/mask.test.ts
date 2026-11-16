import { applyMask } from "./mask";
import { isValid } from "./validators";

describe("applyMask", () => {
  it("formats while typing and accepts pasted formatted values", () => {
    expect(applyMask("000.000.000-00", "12345678909")).toBe("123.456.789-09");
    expect(applyMask("000.000.000-00", "1234")).toBe("123.4");
    expect(applyMask("000.000.000-00", "123.")).toBe("123");
    expect(applyMask("000.000.000-00", "123.456.789-09")).toBe("123.456.789-09");
    expect(applyMask("00000-000", "01310-100 extra")).toBe("01310-100");
    expect(applyMask("(00) 00000-0000", "11987654321")).toBe("(11) 98765-4321");
    expect(applyMask("AA-00", "ab12")).toBe("ab-12");
    expect(applyMask("AA-00", "1a")).toBe("a");
    expect(applyMask("**", "x!y")).toBe("x!");
    expect(applyMask("000", "")).toBe("");
  });
});

describe("isValid", () => {
  it("checks Brazilian documents, cards and IBANs; unknown checks pass", () => {
    expect(isValid("cpf", "529.982.247-25")).toBe(true);
    expect(isValid("cpf", "529.982.247-24")).toBe(false);
    expect(isValid("cpf", "111.111.111-11")).toBe(false);
    expect(isValid("cpf", "123")).toBe(false);
    expect(isValid("cnpj", "11.222.333/0001-81")).toBe(true);
    expect(isValid("cnpj", "11.222.333/0001-80")).toBe(false);
    expect(isValid("cnpj", "00.000.000/0000-00")).toBe(false);
    expect(isValid("cnpj", "11.444.777/0001-61")).toBe(true);
    expect(isValid("luhn", "4111 1111 1111 1111")).toBe(true);
    expect(isValid("luhn", "4111 1111 1111 1112")).toBe(false);
    expect(isValid("luhn", "4111")).toBe(false);
    expect(isValid("iban", "GB82 WEST 1234 5698 7654 32")).toBe(true);
    expect(isValid("iban", "GB82 WEST 1234 5698 7654 33")).toBe(false);
    expect(isValid("iban", "nope")).toBe(false);
    expect(isValid("cpf", "  ")).toBe(true);
    expect(isValid(undefined, "x")).toBe(true);
    expect(isValid("unknown", "x")).toBe(true);
    expect(isValid("nif_pt", "123456789")).toBe(true);
    expect(isValid("nif_pt", "123456788")).toBe(false);
    expect(isValid("nif_pt", "12")).toBe(false);
    expect(isValid("dni_es", "12345678Z")).toBe(true);
    expect(isValid("dni_es", "12345678A")).toBe(false);
    expect(isValid("dni_es", "X1234567L")).toBe(true);
    expect(isValid("dni_es", "hello")).toBe(false);
    expect(isValid("cuit", "20-12345678-6")).toBe(true);
    expect(isValid("cuit", "20-12345678-5")).toBe(false);
    expect(isValid("cuit", "20-1")).toBe(false);
  });
});
