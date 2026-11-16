import { installPasskeys, type PageReply } from "./page";
import { startRelay } from "./relay";

type Fake = Window & typeof globalThis;

// jsdom leaves `source` empty; browsers set it to the posting window.
beforeAll(() => {
  window.postMessage = ((data: unknown) => {
    setTimeout(() => window.dispatchEvent(new MessageEvent("message", { data, source: window, origin: window.location.origin })));
  }) as typeof window.postMessage;
});

let answer: (kind: string, options: Record<string, unknown>) => PageReply | Promise<PageReply> | "reject" = () => ({ fallback: true });
const originalCreate = vi.fn();
const originalGet = vi.fn();
const send = vi.fn(async (message: { kind: string; options: Record<string, unknown> }) => {
  const reply = await answer(message.kind, message.options);
  if (reply === "reject") throw new Error("extension reloaded");
  return { ok: true as const, result: reply };
});

beforeAll(() => {
  Object.defineProperty(navigator, "credentials", { configurable: true, value: { create: originalCreate, get: originalGet } });
  installPasskeys(window as Fake);
  startRelay(window, send as never);
});

function setup(next: typeof answer) {
  answer = next;
  send.mockClear();
  originalCreate.mockReset().mockResolvedValue("browser-create");
  originalGet.mockReset().mockResolvedValue("browser-get");
  return { originalCreate, originalGet, send, credentials: navigator.credentials };
}

const createArgs = {
  publicKey: {
    rp: { name: "Example", id: "localhost" },
    user: { id: new Uint8Array([1]), name: "ana", displayName: "Ana" },
    challenge: new Uint8Array([2]),
    pubKeyCredParams: [{ type: "public-key" as const, alg: -7 }],
  },
};

describe("passkeys in the page", () => {
  it("answers create with Password365's registration", async () => {
    const { credentials, send, originalCreate } = setup(() => ({
      result: { credential_id: "AQ", client_data_json: "Ag", attestation_object: "Aw", authenticator_data: "BA", public_key: "BQ", public_key_algorithm: -7 },
    }));
    const credential = (await credentials.create(createArgs)) as PublicKeyCredential;
    expect(credential.id).toBe("AQ");
    expect(send).toHaveBeenCalledWith({ kind: "passkey-create", options: expect.objectContaining({ rp_id: "localhost", user_name: "ana", challenge: "Ag" }) });
    expect(originalCreate).not.toHaveBeenCalled();
  });

  it("answers get with an assertion and passes everything else to the browser", async () => {
    const { credentials, originalGet } = setup(() => ({
      result: { credential_id: "AQ", client_data_json: "Ag", authenticator_data: "Aw", signature: "BA", user_handle: "BQ" },
    }));
    const credential = (await credentials.get({ publicKey: { challenge: new Uint8Array([3]) } })) as PublicKeyCredential;
    expect([...new Uint8Array((credential.response as AuthenticatorAssertionResponse).signature)]).toEqual([4]);
    expect(await credentials.get({ password: true } as CredentialRequestOptions)).toBe("browser-get");
    expect(originalGet).toHaveBeenCalledTimes(1);
  });

  it("falls back to the browser when declined or unreachable, and reports refusals", async () => {
    let answer: PageReply | "reject" = { fallback: true };
    const { credentials, originalCreate } = setup(() => answer);
    expect(await credentials.create(createArgs)).toBe("browser-create");
    answer = "reject";
    expect(await credentials.create(createArgs)).toBe("browser-create");
    expect(originalCreate).toHaveBeenCalledTimes(2);
    answer = { error: { name: "NotAllowedError", message: "cancelled" } };
    await expect(credentials.create(createArgs)).rejects.toMatchObject({ name: "NotAllowedError" });
    expect(await credentials.create({} as CredentialCreationOptions)).toBe("browser-create");
  });

  it("sends conditional requests to the extension and tells it when the page gives up", async () => {
    let reply: PageReply | Promise<PageReply> = { fallback: true };
    const { credentials, send, originalGet } = setup(() => reply);
    expect(await credentials.get({ publicKey: { challenge: new Uint8Array([3]) }, mediation: "conditional" })).toBe("browser-get");
    expect(send).toHaveBeenCalledWith({ kind: "passkey-conditional", options: expect.objectContaining({ challenge: "Aw" }) });
    expect(originalGet).toHaveBeenCalledTimes(1);

    reply = new Promise(() => {});
    const controller = new AbortController();
    const waiting = credentials.get({ publicKey: { challenge: new Uint8Array([3]) }, mediation: "conditional", signal: controller.signal });
    await new Promise((r) => setTimeout(r, 5));
    controller.abort();
    await expect(waiting).rejects.toMatchObject({ name: "AbortError" });
    await new Promise((r) => setTimeout(r, 5));
    expect(send).toHaveBeenCalledWith({ kind: "passkey-abort" });
  });

  it("honours an aborted signal", async () => {
    const { credentials } = setup(() => new Promise(() => {}));
    const controller = new AbortController();
    controller.abort();
    await expect(credentials.create({ ...createArgs, signal: controller.signal })).rejects.toMatchObject({ name: "AbortError" });
    const later = new AbortController();
    const pending = credentials.get({ publicKey: { challenge: new Uint8Array([1]) }, signal: later.signal });
    later.abort();
    await expect(pending).rejects.toMatchObject({ name: "AbortError" });
  });
});
