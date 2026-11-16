// DTOs mirrored from `core/password365-app/src/dto.rs`. Keep in sync.

export interface AppStatus {
  initialized: boolean;
  unlocked: boolean;
}

export interface Vault {
  id: string;
  name: string;
  item_count: number;
}

/** Category id: "login", "password", "note", "card", "identity", ... (see `Category`). */
export type ItemKind = string;

/** How a category field is typed in. */
export type FieldInput = "text" | "multiline" | "date" | "month_year" | "email" | "phone" | "url" | "totp" | "sign_in_with";

/** One field of a category's template. */
export interface FieldSpec {
  label: string;
  input: FieldInput;
  concealed: boolean;
  /** Section heading ("" = the item's first block). */
  section: string;
  /** What forms call it (HTML autocomplete name or a template token). */
  autofill?: string;
  /** Translation key of the shown name, when it differs from the label ("street"). */
  name?: string;
  /** Input mask: 0 a digit, A a letter, * anything. */
  mask?: string;
  /** Named check of the value ("cpf", "luhn" …). */
  validate?: string;
  /** Values to pick from. */
  options?: string[];
}

/** A category, as on 1Password's "What would you like to add?" screen. */
export interface Category {
  kind: ItemKind;
  label: string;
  plural: string;
  /** Extra fields; empty for logins, passwords and secure notes. */
  fields: FieldSpec[];
  /** Countries with fields of their own ("BR" → fields). */
  regions?: Record<string, FieldSpec[]>;
}

/** A category or extra field of an item. Concealed values are revealed one at a time. */
export interface ItemField {
  label: string;
  concealed: boolean;
  /** Not sent for concealed fields and one-time passwords. */
  value: string | null;
  /** Section heading ("" = the item's first block). */
  section: string;
  input: FieldInput;
}

/** A file attached to an item. */
export interface AttachmentInfo {
  id: string;
  name: string;
  size: number;
  content_type: string;
  /** The bytes are on this device (else they arrive with the next sync). */
  available: boolean;
}

/** One field as the editor saves it. */
export interface FieldValue {
  label: string;
  value: string;
  section?: string;
  input?: FieldInput;
  concealed?: boolean;
}

export interface ItemSummary {
  id: string;
  vault_id: string;
  kind: ItemKind;
  title: string;
  username: string | null;
  urls: string[];
  favorite: boolean;
  tags: string[];
  /** In the Archive (out of the list, search and filling). */
  archived: boolean;
  updated_at: number;
  /** Two passwords were set at once on two devices; the user should review. */
  needs_review: boolean;
}

/** A tag in use and how many items carry it. */
export interface Tag {
  name: string;
  count: number;
}

/** An item in Recently Deleted. */
export interface DeletedItem extends ItemSummary {
  deleted_at: number;
  purge_at: number;
}

export interface Item extends ItemSummary {
  has_password: boolean;
  has_totp: boolean;
  has_notes: boolean;
  password_history_count: number;
  fields: ItemField[];
  /** Country whose fields the item uses ("" = the shared ones). Absent from older backends. */
  region?: string;
  /** The item's own icon (a data URL). */
  icon?: string | null;
  /** Attached files (absent from older backends). */
  attachments?: AttachmentInfo[];
  /** The login's passkey (site and account; never the key). */
  passkey?: { rp_id: string; user_name: string } | null;
  created_at: number;
  version: number;
}

/** Form data. For updates, `undefined` secrets mean "keep", "" means "remove". */
export interface ItemInput {
  vault_id?: string;
  kind?: ItemKind;
  title: string;
  username?: string;
  urls?: string[];
  tags?: string[];
  favorite?: boolean;
  password?: string;
  notes?: string;
  totp?: string;
  /** All category/extra fields in order; omitted = keep the current ones. */
  fields?: FieldValue[];
  /** Country whose fields the item uses ("" = the shared ones); omitted = keep. */
  region?: string;
  /** The item's own icon as a data URL; "" removes it; omitted = keep. */
  icon?: string;
}

export type SecretField = "password" | "notes" | "totp";

/** The master password was changed on another device. */
export interface PasswordUpdate {
  device_name: string;
  changed_at: number;
}

/** A one-time password (two-step verification code). */
export interface TotpCode {
  code: string;
  /** Seconds until it changes. */
  remaining: number;
  /** Seconds each code lasts. */
  period: number;
}

export type CharClass = "Lowercase" | "Uppercase" | "Digits" | "Symbols";

export type GenerateRequest =
  | { type: "password"; length: number; classes: CharClass[]; exclude_ambiguous: boolean }
  | { type: "passphrase"; words: number; separator: string; capitalize: boolean; include_number: boolean }
  | { type: "pin"; digits: number };

export interface Generated {
  value: string;
  bits: number;
  strength: string;
}

export interface ImportSummary {
  imported: number;
  identical: number;
  changed: number;
  updated: number;
  duplicates_in_file: number;
  skipped: [number, string][];
}

/** Strength of a candidate master password (computed by the core with zxcvbn). */
export interface PasswordStrength {
  score: number;
  label: "very_weak" | "weak" | "fair" | "good" | "strong";
  missing_chars: number;
  warning: string | null;
  suggestions: string[];
  acceptable: boolean;
}

/** Backup status (from the core). */
export interface BackupInfo {
  last_backup_ms: number | null;
  /** No backup yet, or the last one is older than 30 days. */
  due: boolean;
}

export type Theme = "system" | "light" | "dark";
export type Density = "comfortable" | "compact";
export type OpenTo = "all_items" | "favorites" | "watchtower";

/** Preferences of this device (Settings). */
export interface Settings {
  auto_lock_minutes: number;
  /** BCP 47 tag; "" follows the system. */
  language: string;
  lock_on_sleep: boolean;
  /** 0 keeps copied text. */
  clipboard_clear_seconds: number;
  theme: Theme;
  density: Density;
  zoom_percent: number;
  sidebar_categories: boolean;
  sidebar_tags: boolean;
  /** "" = the first vault. */
  default_vault: string;
  open_to: OpenTo;
  start_at_login: boolean;
  show_wifi_qr: boolean;
  ssh_agent: boolean;
  /** Ask haveibeenpwned.com about passwords (only hash prefixes are sent). */
  check_breached_passwords: boolean;
  /** Global shortcuts ("Shift+Super+Space"); "" = none. */
  shortcut_quick_access: string;
  shortcut_auto_type: string;
  shortcut_lock: string;
  /** Country for new identities ("BR"); "" = the system's region; "ZZ" = international fields. */
  region: string;
  /** Looking for new versions (off until the first-run question is answered). */
  update_checks: "unasked" | "automatic" | "manual";
  /** A version not to announce automatically ("" for none). */
  skipped_update: string;
}

/** A login whose password appeared in data breaches. */
export interface Breached {
  item: ItemRef;
  count: number;
}

/** An SSH Key item and whether the agent can use it. */
export interface SshKey {
  id: string;
  title: string;
  fingerprint: string | null;
  /** "unreadable" (not an OpenSSH Ed25519 key) or "encrypted" (passphrase missing or wrong). */
  problem: "unreadable" | "encrypted" | null;
}

/** The SSH agent's address and keys. */
export interface SshKeys {
  address: string;
  keys: SshKey[];
}

/** A program wants to sign with an SSH key. */
export interface SshApproval {
  id: number;
  title: string;
  fingerprint: string;
  /** The program asking (file name), when the system tells. */
  program?: string | null;
}

export type ErrorCode =
  | "locked"
  | "not_initialized"
  | "already_initialized"
  | "wrong_credentials"
  | "not_found"
  | "conflict"
  | "validation"
  | "io"
  | "internal"
  | "too_many_attempts"
  | "secret_key_required";

/** This device as others see it. */
export interface ThisDevice {
  id: string;
  name: string;
}

/** A paired device. */
export interface Device {
  id: string;
  name: string;
  paired_at: number;
  last_sync: number | null;
  pending_changes: number;
  addr: string | null;
  /** Missing changes for over a week (or very many): remind the user. */
  out_of_sync: boolean;
}

/** What a sync did here. */
export interface SyncResult {
  device: Device;
  changed: number;
  conflicts: string[];
}

/** An invite to show on the existing device. */
export interface Invite {
  code: string;
  qr_svg: string;
}

/** An item a Watchtower finding points at. */
export interface ItemRef {
  id: string;
  title: string;
  kind: string;
}

/** The Watchtower report (computed on this device). */
export interface Watchtower {
  /** 0–1000; null when no item has a password. */
  score: number | null;
  rating: "excellent" | "very_good" | "good" | "fair" | "poor" | null;
  strength: { strong: number; good: number; weak: number };
  weak: ItemRef[];
  reused: ItemRef[][];
  unsecured: ItemRef[];
  expiring: { item: ItemRef; date: string; expired: boolean }[];
}

/** The auto-type shortcut found no item, or several, for the window in front. */
export interface AutoTypeChoice {
  window: string;
  candidates: { id: string; title: string; username: string | null }[];
}

/** A chosen Profile item. */
export interface ProfileItem {
  id: string;
  title: string;
  last4: string | null;
}

/** The identity and card the browser extension fills into forms. */
export interface Profile {
  identity: ProfileItem | null;
  card: ProfileItem | null;
}

export type ProfileKind = "identity" | "card";

/** A newer version of Password365 (Settings › About, the update banner). */
export interface UpdateInfo {
  version: string;
  notes: string | null;
}

/** Help and feedback destinations (Settings › About). */
export type HelpTopic = "bug" | "idea" | "translate" | "security" | "email";
