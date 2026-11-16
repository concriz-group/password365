// Input masks from the country templates: `0` a digit, `A` a letter, `*`
// anything; other characters are inserted as typed separators.

const SLOT: Record<string, RegExp> = { "0": /\d/, A: /\p{L}/u, "*": /./u };

/** `raw` shaped by `mask` ("12345678909" + "000.000.000-00" → "123.456.789-09"). Extra input is dropped. */
export function applyMask(mask: string, raw: string): string {
  const literals = new Set([...mask].filter((c) => !SLOT[c]));
  const chars = [...raw].filter((c) => !literals.has(c) || /[\p{L}\d]/u.test(c));
  let out = "";
  let pending = "";
  for (const m of mask) {
    if (chars.length === 0) break;
    const slot = SLOT[m];
    if (!slot) {
      pending += m;
      continue;
    }
    while (chars.length > 0 && !slot.test(chars[0]!)) chars.shift();
    const c = chars.shift();
    if (c === undefined) break;
    out += pending + c;
    pending = "";
  }
  return out;
}
