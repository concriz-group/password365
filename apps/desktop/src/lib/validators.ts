// Checks named by the country templates (`"validate": "cpf"`). A failed check
// is only a warning: the value is still saved as typed.

const digits = (value: string) => value.replace(/\D/g, "");

/** Brazilian CPF check digits. */
function cpf(value: string): boolean {
  const d = digits(value);
  if (d.length !== 11 || /^(\d)\1+$/.test(d)) return false;
  const check = (len: number) => {
    const sum = [...d.slice(0, len)].reduce((s, c, i) => s + Number(c) * (len + 1 - i), 0);
    return ((sum * 10) % 11) % 10;
  };
  return check(9) === Number(d[9]) && check(10) === Number(d[10]);
}

/** Brazilian CNPJ check digits. */
function cnpj(value: string): boolean {
  const d = digits(value);
  if (d.length !== 14 || /^(\d)\1+$/.test(d)) return false;
  const check = (len: number) => {
    const weights = len === 12 ? [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2] : [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    const rest = [...d.slice(0, len)].reduce((s, c, i) => s + Number(c) * weights[i]!, 0) % 11;
    return rest < 2 ? 0 : 11 - rest;
  };
  return check(12) === Number(d[12]) && check(13) === Number(d[13]);
}

/** Luhn checksum (card numbers). */
function luhn(value: string): boolean {
  const d = digits(value);
  if (d.length < 12) return false;
  const sum = [...d].reverse().reduce((s, c, i) => {
    const n = Number(c) * (i % 2 ? 2 : 1);
    return s + (n > 9 ? n - 9 : n);
  }, 0);
  return sum % 10 === 0;
}

/** IBAN mod-97 check. */
function iban(value: string): boolean {
  const v = value.replace(/\s/g, "").toUpperCase();
  if (!/^[A-Z]{2}\d{2}[A-Z0-9]{10,30}$/.test(v)) return false;
  const moved = (v.slice(4) + v.slice(0, 4)).replace(/[A-Z]/g, (c) => String(c.charCodeAt(0) - 55));
  return [...moved].reduce((r, c) => (r * 10 + Number(c)) % 97, 0) === 1;
}

/** Portuguese NIF: mod-11 check digit. */
function nifPt(value: string): boolean {
  const d = digits(value);
  if (d.length !== 9) return false;
  const sum = [...d.slice(0, 8)].reduce((s, c, i) => s + Number(c) * (9 - i), 0);
  const check = 11 - (sum % 11);
  return (check >= 10 ? 0 : check) === Number(d[8]);
}

/** Spanish DNI (8 digits + letter) or NIE (X/Y/Z + 7 digits + letter). */
function dniEs(value: string): boolean {
  const v = value.replace(/[\s-]/g, "").toUpperCase();
  const m = /^([XYZ]?)(\d{7,8})([A-Z])$/.exec(v);
  if (!m) return false;
  const number = Number(`${"XYZ".indexOf(m[1]!) >= 0 && m[1] ? "XYZ".indexOf(m[1]!) : ""}${m[2]}`);
  return "TRWAGMYFPDXBNJZSQVHLCKE"[number % 23] === m[3];
}

/** Argentine CUIT/CUIL: mod-11 with weights 5432765432. */
function cuit(value: string): boolean {
  const d = digits(value);
  if (d.length !== 11) return false;
  const weights = [5, 4, 3, 2, 7, 6, 5, 4, 3, 2];
  const rest = 11 - (weights.reduce((s, w, i) => s + w * Number(d[i]), 0) % 11);
  const check = rest === 11 ? 0 : rest === 10 ? 9 : rest;
  return check === Number(d[10]);
}

const VALIDATORS: Record<string, (value: string) => boolean> = { cpf, cnpj, luhn, iban, nif_pt: nifPt, dni_es: dniEs, cuit };

/** `false` only when `name` is a known check and `value` (not empty) fails it. */
export function isValid(name: string | undefined, value: string): boolean {
  const check = name ? VALIDATORS[name] : undefined;
  return !check || value.trim() === "" || check(value);
}
