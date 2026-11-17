// Converting WebAuthn options and answers between the page (ArrayBuffers) and
// the extension (base64url text). Runs in the page's own context, so it uses
// nothing but the language and the DOM.

export type Bytes = ArrayBuffer | ArrayBufferView;

export function toB64url(bytes: Bytes): string {
  const view = bytes instanceof ArrayBuffer ? new Uint8Array(bytes) : new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let text = "";
  for (const b of view) text += String.fromCharCode(b);
  return btoa(text).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function fromB64url(text: string): ArrayBuffer {
  const base64 = text.replace(/-/g, "+").replace(/_/g, "/");
  const raw = atob(base64 + "=".repeat((4 - (base64.length % 4)) % 4));
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
  return out.buffer;
}

/** What `create` asks, as text (the app's `CreateRequest` plus display bits). */
export interface CreateOptions {
  rp_id: string;
  rp_name: string;
  user_id: string;
  user_name: string;
  challenge: string;
  algorithms: number[];
  exclude: string[];
  user_verification: UserVerification;
}

/** The page's `userVerification` (absent: "preferred"). */
export type UserVerification = "required" | "preferred" | "discouraged";

function verificationOf(value: string | undefined): UserVerification {
  return value === "required" || value === "discouraged" ? value : "preferred";
}

/** What `get` asks, as text. */
export interface GetOptions {
  rp_id: string;
  challenge: string;
  allow: string[];
  user_verification: UserVerification;
}

export function createOptions(options: PublicKeyCredentialCreationOptions, host: string): CreateOptions {
  return {
    rp_id: options.rp.id ?? host,
    rp_name: options.rp.name ?? "",
    user_id: toB64url(options.user.id),
    user_name: options.user.name ?? options.user.displayName ?? "",
    challenge: toB64url(options.challenge),
    algorithms: (options.pubKeyCredParams ?? []).map((p) => p.alg),
    exclude: (options.excludeCredentials ?? []).map((c) => toB64url(c.id)),
    user_verification: verificationOf(options.authenticatorSelection?.userVerification),
  };
}

export function getOptions(options: PublicKeyCredentialRequestOptions, host: string): GetOptions {
  return {
    rp_id: options.rpId ?? host,
    challenge: toB64url(options.challenge),
    allow: (options.allowCredentials ?? []).map((c) => toB64url(c.id)),
    user_verification: verificationOf(options.userVerification),
  };
}

/** The app's answer to `create` (binary values base64url). */
export interface Registration {
  credential_id: string;
  client_data_json: string;
  attestation_object: string;
  authenticator_data: string;
  public_key: string;
  public_key_algorithm: number;
}

/** The app's answer to `get`. */
export interface Assertion {
  credential_id: string;
  client_data_json: string;
  authenticator_data: string;
  signature: string;
  user_handle: string;
}

/** Gives `object` the browser's prototype (so `instanceof` checks pass) when it exists. */
function like<T extends object>(object: T, proto: { prototype: object } | undefined): T {
  if (proto) Object.setPrototypeOf(object, proto.prototype);
  return object;
}

function define(target: object, values: Record<string, unknown>) {
  for (const [key, value] of Object.entries(values)) {
    Object.defineProperty(target, key, { value, enumerable: true, configurable: true });
  }
}

/** A `PublicKeyCredential` for a registration, as the page expects it. */
export function registrationCredential(r: Registration, globals: Partial<Record<string, { prototype: object }>> = globalThis as never) {
  const response = like({}, globals.AuthenticatorAttestationResponse);
  define(response, {
    clientDataJSON: fromB64url(r.client_data_json),
    attestationObject: fromB64url(r.attestation_object),
    getAuthenticatorData: () => fromB64url(r.authenticator_data),
    getPublicKey: () => fromB64url(r.public_key),
    getPublicKeyAlgorithm: () => r.public_key_algorithm,
    getTransports: () => ["hybrid", "internal"],
  });
  const json = {
    id: r.credential_id,
    rawId: r.credential_id,
    type: "public-key",
    authenticatorAttachment: "platform",
    clientExtensionResults: {},
    response: {
      clientDataJSON: r.client_data_json,
      attestationObject: r.attestation_object,
      authenticatorData: r.authenticator_data,
      publicKey: r.public_key,
      publicKeyAlgorithm: r.public_key_algorithm,
      transports: ["hybrid", "internal"],
    },
  };
  return credential(r.credential_id, response, json, globals);
}

/** A `PublicKeyCredential` for a sign-in. */
export function assertionCredential(a: Assertion, globals: Partial<Record<string, { prototype: object }>> = globalThis as never) {
  const response = like({}, globals.AuthenticatorAssertionResponse);
  define(response, {
    clientDataJSON: fromB64url(a.client_data_json),
    authenticatorData: fromB64url(a.authenticator_data),
    signature: fromB64url(a.signature),
    userHandle: fromB64url(a.user_handle),
  });
  const json = {
    id: a.credential_id,
    rawId: a.credential_id,
    type: "public-key",
    authenticatorAttachment: "platform",
    clientExtensionResults: {},
    response: {
      clientDataJSON: a.client_data_json,
      authenticatorData: a.authenticator_data,
      signature: a.signature,
      userHandle: a.user_handle,
    },
  };
  return credential(a.credential_id, response, json, globals);
}

function credential(id: string, response: object, json: object, globals: Partial<Record<string, { prototype: object }>>) {
  const out = like({}, globals.PublicKeyCredential);
  define(out, {
    id,
    rawId: fromB64url(id),
    type: "public-key",
    authenticatorAttachment: "platform",
    response,
    getClientExtensionResults: () => ({}),
    toJSON: () => json,
  });
  return out as PublicKeyCredential;
}
