import type { Category, Item, ItemSummary, Vault } from "../api/types";

export const personal: Vault = { id: "v1", name: "Personal", item_count: 1 };
export const work: Vault = { id: "v2", name: "Work", item_count: 0 };

export const summary: ItemSummary = {
  id: "i1",
  vault_id: "v1",
  kind: "login",
  title: "GitHub",
  username: "octo",
  urls: ["https://github.com"],
  favorite: false,
  tags: ["dev"],
  archived: false,
  updated_at: 0,
  needs_review: false,
};

export const item: Item = {
  ...summary,
  has_password: true,
  has_totp: true,
  has_notes: true,
  password_history_count: 2,
  fields: [],
  created_at: 0,
  version: 3,
};

export const categories: Category[] = [
  { kind: "login", label: "Login", plural: "Logins", fields: [] },
  { kind: "password", label: "Password", plural: "Passwords", fields: [] },
  { kind: "note", label: "Secure Note", plural: "Secure Notes", fields: [] },
  {
    kind: "card",
    label: "Credit Card",
    plural: "Credit Cards",
    fields: [
      { label: "cardholder name", input: "text", concealed: false, section: "" },
      { label: "number", input: "text", concealed: true, section: "" },
      { label: "expiry date", input: "month_year", concealed: false, section: "" },
    ],
  },
  { kind: "identity", label: "Identity", plural: "Identities", fields: [{ label: "address", input: "multiline", concealed: false, section: "" }] },
  { kind: "api_credential", label: "API Credential", plural: "API Credentials", fields: [] },
  { kind: "wireless_router", label: "Wireless Router", plural: "Wireless Routers", fields: [] },
];
