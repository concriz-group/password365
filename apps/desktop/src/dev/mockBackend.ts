import { roughStrength } from "./roughStrength";
import { copySecret } from "../lib/clipboard";
// DEV ONLY: in-memory fake backend so the UI can be developed and reviewed in a
// normal browser (`npm run dev`, open http://localhost:1420). Never bundled into
// the Tauri app (see main.tsx). Contains only fake sample data.

import type { Invoke } from "../api/client";
import type { Category, Device, FieldSpec, Item, ItemInput, Settings, Vault } from "../api/types";
import { DEFAULT_SETTINGS } from "../settings";

type Stored = Item & { password?: string; notes?: string; totp?: string; secrets: Record<string, string> };

/** A few categories (the real list and fields come from the Rust core). */
const CATEGORIES: Category[] = [
  { kind: "login", label: "Login", plural: "Logins", fields: [] },
  { kind: "password", label: "Password", plural: "Passwords", fields: [] },
  { kind: "note", label: "Secure Note", plural: "Secure Notes", fields: [] },
  {
    kind: "card",
    label: "Credit Card",
    plural: "Credit Cards",
    fields: [
      { label: "cardholder name", input: "text", concealed: false, section: "" },
      { label: "type", input: "text", concealed: false, section: "" },
      { label: "number", input: "text", concealed: true, section: "" },
      { label: "verification number", input: "text", concealed: true, section: "" },
      { label: "expiry date", input: "month_year", concealed: false, section: "" },
    ],
  },
  {
    kind: "identity",
    label: "Identity",
    plural: "Identities",
    fields: [
      { label: "first name", input: "text", concealed: false, section: "Identification" },
      { label: "last name", input: "text", concealed: false, section: "Identification" },
      { label: "birth date", input: "date", concealed: false, section: "Identification" },
      { label: "address", input: "multiline", concealed: false, section: "Address" },
      { label: "phone", input: "phone", concealed: false, section: "Address" },
      { label: "email", input: "email", concealed: false, section: "Internet Details" },
    ],
  },
  {
    kind: "api_credential",
    label: "API Credential",
    plural: "API Credentials",
    fields: [
      { label: "username", input: "text", concealed: false, section: "" },
      { label: "credential", input: "text", concealed: true, section: "" },
      { label: "hostname", input: "text", concealed: false, section: "" },
    ],
  },
  { kind: "ssh_key", label: "SSH Key", plural: "SSH Keys", fields: [{ label: "private key", input: "multiline", concealed: true, section: "" }] },
  { kind: "document", label: "Document", plural: "Documents", fields: [] },
  { kind: "wireless_router", label: "Wireless Router", plural: "Wireless Routers", fields: [{ label: "network name", input: "text", concealed: false, section: "" }, { label: "wireless network password", input: "text", concealed: true, section: "" }] },
];

// The real templates (templates/categories.json and every country file).
import baseTemplates from "../../../../templates/categories.json";
const regionFiles = import.meta.glob("../../../../templates/regions/*.json", { eager: true, import: "default" });
for (const category of CATEGORIES) {
  const shared = (baseTemplates as Record<string, unknown>)[category.kind];
  if (Array.isArray(shared)) category.fields = shared.map((f) => ({ input: "text", concealed: false, section: "", ...f }));
  const regions: Record<string, FieldSpec[]> = {};
  for (const [path, file] of Object.entries(regionFiles)) {
    const fields = (file as { categories?: Record<string, FieldSpec[]> }).categories?.[category.kind];
    if (fields) regions[/([A-Z]{2})\.json$/.exec(path)![1]!] = fields.map((f) => ({ ...f, input: f.input ?? "text", concealed: f.concealed ?? false, section: f.section ?? "" }));
  }
  if (Object.keys(regions).length > 0) category.regions = regions;
}

/** Stores form fields: concealed values go to `secrets`, the rest are shown. */
function storeFields(kind: string, input: ItemInput, item: Pick<Stored, "fields" | "secrets" | "region" | "icon">) {
  if (input.region !== undefined) item.region = input.region;
  if (input.icon !== undefined) item.icon = input.icon || null;
  if (!input.fields) return;
  const category = CATEGORIES.find((c) => c.kind === kind);
  const template = category?.regions?.[item.region ?? ""] ?? category?.fields ?? [];
  item.fields = [];
  item.secrets = {};
  for (const f of input.fields.filter((f) => f.value)) {
    const spec = template.find((t) => t.label === f.label);
    const concealed = f.concealed ?? spec?.concealed ?? false;
    const fieldInput = f.input ?? spec?.input ?? "text";
    const hidden = concealed || fieldInput === "totp";
    item.fields.push({ label: f.label, concealed, value: hidden ? null : f.value, section: f.section ?? spec?.section ?? "", input: fieldInput });
    if (hidden) item.secrets[f.label] = f.value;
  }
}

export function createMockBackend(): Invoke {
  let initialized = true;
  let unlocked = false;
  const vaults: Vault[] = [
    { id: "v-personal", name: "Personal", item_count: 0 },
    { id: "v-work", name: "Work", item_count: 0 },
  ];
  const items: Stored[] = [];
  const deleted: (Stored & { deleted_at: number })[] = [];
  const RETENTION_MS = 30 * 24 * 60 * 60 * 1000;
  let seq = 0;
  let settings: Settings = { ...DEFAULT_SETTINGS };

  function add(vault_id: string, input: ItemInput): Stored {
    seq += 1;
    const item: Stored = {
      id: `item-${seq}`,
      vault_id,
      kind: input.kind ?? "login",
      title: input.title,
      username: input.username || null,
      urls: input.urls ?? [],
      favorite: false,
      archived: false,
      updated_at: Date.now(),
      needs_review: false,
      tags: input.tags ?? [],
      has_password: !!input.password,
      has_totp: !!input.totp,
      has_notes: !!input.notes,
      password_history_count: 0,
      fields: [],
      secrets: {},
      created_at: Date.now(),
      version: 1,
      password: input.password,
      notes: input.notes,
      totp: input.totp,
    };
    storeFields(item.kind, input, item);
    items.push(item);
    return item;
  }

  add("v-personal", { title: "GitHub", username: "octocat", urls: ["https://github.com"], password: "demo-Pa55word!", tags: ["dev", "Starter Kit"], notes: "Recovery codes are in the safe." });
  add("v-personal", { title: "Bank", username: "alice", urls: ["https://bank.example"], password: "another demo", totp: "JBSWY3DPEHPK3PXP" });
  add("v-work", { title: "Jira", username: "alice@corp.example", urls: ["https://jira.example"], password: "work demo" });
  add("v-personal", { kind: "note", title: "Wi-Fi at home", notes: "SSID: Castle\nKey: demo-wifi" });
  add("v-personal", {
    kind: "card",
    title: "Demo Visa",
    fields: [
      { label: "cardholder name", value: "Alice Example" },
      { label: "number", value: "4111111111111111" },
      { label: "expiry date", value: "2029-04" },
    ],
  });

  add("v-personal", {
    kind: "identity",
    title: "Alice Example",
    fields: [
      { label: "first name", value: "Alice" },
      { label: "address", value: "1 Demo Street\nSpringfield" },
      { label: "spouse", value: "Bob Example", section: "Family", input: "text" },
      { label: "bank 2fa", value: "JBSWY3DPEHPK3PXP", section: "Family", input: "totp" },
    ],
  });

  add("v-personal", {
    kind: "wireless_router",
    title: "Home Wi-Fi",
    fields: [
      { label: "network name", value: "Castle" },
      { label: "wireless network password", value: "demo-wifi" },
    ],
  });

  let lastBackup: number | null = null;
  const profile: { identity: string | null; card: string | null } = { identity: null, card: null };
  let deviceName = "This Mac";
  const devices: Device[] = [
    { id: "d-phone", name: "Alice's phone", paired_at: Date.now() - 86_400_000 * 30, last_sync: Date.now() - 3_600_000 * 5, pending_changes: 2, addr: "192.168.1.23:47480", out_of_sync: false },
  ];
  const fail = (code: string, message: string) => Promise.reject({ code, message });
  const counts = () => vaults.map((v) => ({ ...v, item_count: items.filter((i) => i.vault_id === v.id && !i.archived).length }));
  const strip = ({ password: _p, notes: _n, totp: _t, secrets: _s, ...rest }: Stored): Item => rest;

  return async (command, args = {}) => {
    const a = args as Record<string, unknown>;
    if (command === "status") return { initialized, unlocked };
    if (command === "print_page") return window.print();
    if (command === "backup_info") return { last_backup_ms: lastBackup, due: lastBackup === null };
    if (command === "restore_backup") return fail("io", "restoring a backup needs the desktop app");
    if (command === "password_strength") return roughStrength(String(a.password ?? ""));
    if (command === "generate") return { value: "Demo-Generated-Pw-42!", bits: 130, strength: "excellent" };
    if (command === "create_account") {
      initialized = true;
      unlocked = true;
      return "P365-DEMO-DEMO-DEMO-DEMO-DEMO-DEMO-DEMO";
    }
    if (command === "unlock") {
      if (a.password !== "demo") return fail("wrong_credentials", "wrong master password or Secret Key (demo password: demo)");
      unlocked = true;
      return null;
    }
    if (!unlocked) return fail("locked", "the vault is locked");
    switch (command) {
      case "settings":
        return settings;
      case "update_settings":
        settings = a.settings as Settings;
        return null;
      case "pending_password_change":
        return null;
      case "adopt_master_password":
        return null;
      case "save_emergency_kit":
        return a.password === "demo" ? "kit-demo" : fail("wrong_credentials", "wrong master password (demo: demo)");
      case "reveal_secret_key":
        return a.password === "demo" ? "P365-DEMO-DEMO-DEMO-DEMO-DEMO-DEMO-DEMO" : fail("wrong_credentials", "wrong master password (demo: demo)");
      case "change_master_password":
        return a.current === "demo" ? null : fail("wrong_credentials", "wrong master password (demo: demo)");
      case "user_active":
        return undefined;
      case "lock":
        unlocked = false;
        return null;
      case "vaults":
        return counts();
      case "create_vault": {
        const v = { id: `v-${Date.now()}`, name: String(a.name), item_count: 0 };
        vaults.push(v);
        return v;
      }
      case "items":
      case "archived_items": {
        const q = String(a.query).toLowerCase();
        return items
          .filter((i) => i.archived === (command === "archived_items"))
          .filter((i) => !a.vaultId || i.vault_id === a.vaultId)
          .filter((i) => [i.title, i.username ?? "", ...i.urls].some((f) => f.toLowerCase().includes(q)))
          .sort((x, y) => x.title.localeCompare(y.title))
          .map(strip);
      }
      case "item": {
        const found = items.find((i) => i.id === a.id);
        return found ? strip(found) : fail("not_found", "item not found");
      }
      case "reveal": {
        const found = items.find((i) => i.id === a.id);
        return found?.[a.field as "password" | "notes" | "totp"] ?? null;
      }
      case "totp_code": {
        const found = items.find((i) => i.id === a.id);
        if (a.label ? !found?.secrets[a.label as string] : !found?.totp) return null;
        const now = Math.floor(Date.now() / 1000);
        const code = String((Math.floor(now / 30) * 7919) % 1_000_000).padStart(6, "0");
        return { code, remaining: 30 - (now % 30), period: 30 };
      }
      case "create_item": {
        const input = a.input as ItemInput;
        return strip(add(input.vault_id ?? "v-personal", input));
      }
      case "update_item": {
        const input = a.input as ItemInput;
        const found = items.find((i) => i.id === a.id);
        if (!found) return fail("not_found", "item not found");
        Object.assign(found, {
          title: input.title,
          username: input.username || null,
          urls: input.urls ?? [],
          tags: input.tags ?? [],
          version: found.version + 1,
          updated_at: Date.now(),
        });
        if (input.password !== undefined) {
          found.password = input.password;
          found.has_password = !!input.password;
          found.password_history_count += 1;
        }
        if (input.notes !== undefined) {
          found.notes = input.notes;
          found.has_notes = !!input.notes;
        }
        storeFields(found.kind, input, found);
        return strip(found);
      }
      case "delete_item": {
        const index = items.findIndex((i) => i.id === a.id);
        if (index < 0) return fail("not_found", "item not found");
        const [gone] = items.splice(index, 1);
        deleted.unshift({ ...gone!, deleted_at: Date.now() });
        return null;
      }
      case "deleted_items":
        return deleted.map((d) => ({ ...strip(d), deleted_at: d.deleted_at, purge_at: d.deleted_at + RETENTION_MS }));
      case "restore_item": {
        const index = deleted.findIndex((d) => d.id === a.id);
        if (index < 0) return fail("not_found", "deleted item not found");
        const { deleted_at: _d, ...item } = deleted.splice(index, 1)[0]!;
        items.push(item);
        return null;
      }
      case "delete_permanently":
        deleted.splice(deleted.findIndex((d) => d.id === a.id), 1);
        return null;
      case "this_device":
        return { id: "d-this", name: deviceName };
      case "rename_this_device":
        deviceName = String(a.name).trim() || deviceName;
        return { id: "d-this", name: deviceName };
      case "devices":
        return devices;
      case "remove_device":
        devices.splice(devices.findIndex((d) => d.id === a.id), 1);
        return null;
      case "sync_device": {
        const device = devices.find((d) => d.id === a.id);
        if (!device) return fail("not_found", "device not found");
        Object.assign(device, { last_sync: Date.now(), pending_changes: 0, out_of_sync: false });
        return { device, changed: 0, conflicts: [] };
      }
      case "invite_device":
        return {
          code: "PASSWORD365-PAIR:1:DEMODEMODEMO",
          qr_svg: '<svg xmlns="http://www.w3.org/2000/svg" width="220" height="220"><rect width="220" height="220" fill="#fff"/><text x="20" y="115">demo QR</text></svg>',
        };
      case "wait_for_new_device":
        return new Promise((resolve) => setTimeout(() => resolve(undefined), 2500));
      case "approve_new_device": {
        const added = { id: `d-${Date.now()}`, name: "New laptop", paired_at: Date.now(), last_sync: Date.now(), pending_changes: 0, addr: null, out_of_sync: false };
        devices.push(added);
        return added;
      }
      case "tags": {
        const counts = new Map<string, number>();
        for (const tag of items.filter((i) => !i.archived).flatMap((i) => i.tags)) counts.set(tag, (counts.get(tag) ?? 0) + 1);
        return [...counts].map(([name, count]) => ({ name, count })).sort((x, y) => x.name.localeCompare(y.name));
      }
      case "categories":
        return CATEGORIES;
      case "profile": {
        const pick = (kind: string, chosen: string | null) => {
          const all = items.filter((i) => i.kind === kind && !i.archived);
          const item = all.find((i) => i.id === chosen) ?? (all.length === 1 ? all[0] : undefined);
          if (!item) return null;
          const number = item.secrets["number"];
          return { id: item.id, title: item.title, last4: number ? number.slice(-4) : null };
        };
        return { identity: pick("identity", profile.identity), card: pick("card", profile.card) };
      }
      case "set_profile":
        profile[a.kind as "identity" | "card"] = (a.id as string | null) ?? null;
        return null;
      case "auto_type":
        return fail("io", "auto-type needs the desktop app");
      case "watchtower": {
        const ref = (i: Stored) => ({ id: i.id, title: i.title, kind: i.kind });
        const active = items.filter((i) => !i.archived);
        const withPw = active.filter((i) => i.password);
        const weak = withPw.filter((i) => roughStrength(i.password!).score < 3);
        return {
          score: withPw.length ? Math.round((1000 * (withPw.length - weak.length)) / withPw.length) : null,
          rating: withPw.length ? (weak.length ? "good" : "excellent") : null,
          strength: { strong: withPw.length - weak.length, good: 0, weak: weak.length },
          weak: weak.map(ref),
          reused: [],
          unsecured: active.filter((i) => i.urls.some((u) => u.startsWith("http://"))).map(ref),
          expiring: [],
        };
      }
      case "reveal_field": {
        const found = items.find((i) => i.id === a.id);
        const label = String(a.label);
        return found?.secrets[label] ?? found?.fields.find((f) => f.label === label)?.value ?? null;
      }
      case "set_archived": {
        const found = items.find((i) => i.id === a.id);
        if (found) found.archived = Boolean(a.archived);
        return null;
      }
      case "set_favorite": {
        const found = items.find((i) => i.id === a.id);
        if (found) found.favorite = Boolean(a.favorite);
        return null;
      }
      case "mark_reviewed": {
        const found = items.find((i) => i.id === a.id);
        if (found) found.needs_review = false;
        return null;
      }
      case "cancel_pairing":
        return null;
      case "move_item": {
        const found = items.find((i) => i.id === a.id);
        if (!found) return fail("not_found", "item not found");
        found.vault_id = String(a.vaultId);
        return strip(found);
      }
      case "duplicate_item": {
        const found = items.find((i) => i.id === a.id);
        if (!found) return fail("not_found", "item not found");
        const copy: Stored = { ...structuredClone(found), id: `i${items.length + 1}`, title: String(a.title), favorite: false };
        items.push(copy);
        return strip(copy);
      }
      case "item_icons":
        return Object.fromEntries(items.filter((i) => i.icon).map((i) => [i.id, i.icon]));
      case "wifi_qr":
        return '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#fff"/><rect x="1" y="1" width="3" height="3"/><rect x="6" y="1" width="3" height="3"/><rect x="1" y="6" width="3" height="3"/></svg>';
      case "ssh_keys":
        return { address: "~/.password365/agent.sock", keys: items.filter((i) => i.kind === "ssh_key").map((i) => ({ id: i.id, title: i.title, fingerprint: "SHA256:demo", problem: null })) };
      case "attach_file": {
        const found = items.find((i) => i.id === a.id);
        if (!found) return fail("not_found", "item not found");
        const name = String(a.path).split(/[\\/]/).pop() ?? "file";
        const file = { id: `att-${Date.now()}`, name, size: 1234, content_type: "", available: true };
        found.attachments = [...(found.attachments ?? []), file];
        return file;
      }
      case "remove_attachment": {
        const found = items.find((i) => i.id === a.id);
        if (found) found.attachments = (found.attachments ?? []).filter((f) => f.id !== a.attachmentId);
        return null;
      }
      case "import_cxf":
        return { imported: 0, identical: 0, changed: 0, updated: 0, duplicates_in_file: 0, skipped: [] };
      case "export_cxf":
        return items.length;
      case "remove_passkey": {
        const found = items.find((i) => i.id === a.id);
        if (found) found.passkey = null;
        return null;
      }
      case "open_attachment":
      case "save_attachment":
        return null;
      case "copy_secret":
        await copySecret(String(a.text), undefined, undefined, Number(a.clearAfterMs));
        return null;
      case "ssh_answer":
      case "hide_quick_access":
      case "open_in_app":
        return null;
      // A pretend newer version, so the update screens can be looked at (?update=0.2.0).
      case "update_check": {
        const version = new URLSearchParams(location.search).get("update");
        return version ? { version, notes: null } : null;
      }
      case "update_install":
      case "open_help":
        return null;
      case "breached_passwords":
        return items.filter((i) => i.password === "another demo").map((i) => ({ item: { id: i.id, title: i.title, kind: i.kind }, count: 3 }));
      case "export_backup":
        lastBackup = Date.now();
        return { last_backup_ms: lastBackup, due: false };
      default:
        return fail("internal", `demo backend does not implement ${command}`);
    }
  };
}
