import type { CharClass, GenerateRequest } from "../api/types";

export const ALL_CLASSES: CharClass[] = ["Lowercase", "Uppercase", "Digits", "Symbols"];

/** Default strong password: 20 characters from every class (~130 bits). */
export const DEFAULT_PASSWORD: GenerateRequest = {
  type: "password",
  length: 20,
  classes: ALL_CLASSES,
  exclude_ambiguous: false,
};

/** Default memorable passphrase: 5 words + digit (~70 bits). */
export const DEFAULT_PASSPHRASE: GenerateRequest = {
  type: "passphrase",
  words: 5,
  separator: "-",
  capitalize: true,
  include_number: true,
};

export const DEFAULT_PIN: GenerateRequest = { type: "pin", digits: 6 };
