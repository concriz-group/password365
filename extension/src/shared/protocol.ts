// Messages exchanged with the Password365 app through the native-messaging host.
// Mirrors `core/password365-bridge/src/protocol.rs`.

export const HOST_NAME = "app.password365.host";

export interface SubmittedLogin {
  page_url: string;
  username?: string;
  password: string;
  title?: string;
}

export type BridgeBody =
  | { type: "status" }
  | { type: "unlock"; password: string }
  | { type: "logins_for_url"; page_url: string }
  | { type: "fill"; item_id: string; page_url: string }
  | { type: "check_submitted"; login: SubmittedLogin }
  | { type: "save_submitted"; login: SubmittedLogin; vault_id?: string }
  | { type: "generate"; request: GenerateRequest }
  | { type: "profile" }
  | { type: "profile_fill"; profile: ProfileKind; item_id: string | null; page_url: string }
  | {
      type: "passkey_register";
      page_url: string;
      title: string;
      request: {
        rp_id: string;
        user_id: string;
        user_name: string;
        challenge: string;
        algorithms: number[];
        user_verification: "required" | "preferred" | "discouraged";
      };
      exclude: string[];
      /** The master password, when the site requires the user verified. */
      password?: string;
    }
  | { type: "passkey_options"; page_url: string; rp_id: string; allow: string[] }
  | {
      type: "passkey_assert";
      item_id: string;
      page_url: string;
      challenge: string;
      user_verification: "required" | "preferred" | "discouraged";
      /** The master password, when the site requires the user verified. */
      password?: string;
    };

export type ProfileKind = "identity" | "card";

/** A Profile item as the menu shows it (no secrets). */
export interface ProfileItem {
  id: string;
  title: string;
  last4: string | null;
}

/** The default identity and card chosen in the app. */
export interface Profile {
  identity: ProfileItem | null;
  card: ProfileItem | null;
  /** Every identity, the default first (absent from older apps). */
  identities?: ProfileItem[];
  /** Every payment card, the default first. */
  cards?: ProfileItem[];
}

export type GenerateRequest = {
  type: "password";
  length: number;
  classes: ("Lowercase" | "Uppercase" | "Digits" | "Symbols")[];
  exclude_ambiguous: boolean;
};

export const DEFAULT_GENERATE: GenerateRequest = {
  type: "password",
  length: 20,
  classes: ["Lowercase", "Uppercase", "Digits", "Symbols"],
  exclude_ambiguous: false,
};

export interface BridgeResponse {
  id: number;
  ok: boolean;
  result?: unknown;
  error?: { code: string; message: string };
}

export interface AppStatus {
  initialized: boolean;
  unlocked: boolean;
}

export interface LoginCandidate {
  id: string;
  title: string;
  username: string | null;
  match_level: "origin" | "host" | "site";
}

export interface FillCredentials {
  username: string | null;
  password: string | null;
}

export type SubmissionCheck =
  | { status: "new" }
  | { status: "known"; id: string }
  | { status: "password_changed"; id: string };

export interface Generated {
  value: string;
  bits: number;
  strength: string;
}

/** Error with a machine-readable code (`locked`, `app_not_running`, `host_missing`, …). */
export class Password365Error extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "Password365Error";
    this.code = code;
  }
}
