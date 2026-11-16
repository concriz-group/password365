import { assertionCredential, createOptions, fromB64url, getOptions, registrationCredential, toB64url } from "./codec";

const bytes = (...b: number[]) => new Uint8Array(b);

describe("passkey codec", () => {
  it("round-trips base64url without padding", () => {
    for (const sample of [[], [0], [251, 255], [1, 2, 3, 4, 5]]) {
      const text = toB64url(bytes(...sample));
      expect(text).not.toMatch(/[+/=]/);
      expect([...new Uint8Array(fromB64url(text))]).toEqual(sample);
    }
    expect(toB64url(bytes(251, 255))).toBe("-_8");
    const view = new Uint8Array([9, 1, 2, 9]).subarray(1, 3);
    expect(toB64url(view)).toBe(toB64url(bytes(1, 2)));
  });

  it("turns create and get options into text, defaulting the site to the page", () => {
    const create = createOptions(
      {
        rp: { name: "Example" },
        user: { id: bytes(1), name: "ana", displayName: "Ana" },
        challenge: bytes(2),
        pubKeyCredParams: [{ type: "public-key", alg: -8 }, { type: "public-key", alg: -7 }],
        excludeCredentials: [{ type: "public-key", id: bytes(3) }],
      },
      "login.example.com",
    );
    expect(create).toEqual({ rp_id: "login.example.com", rp_name: "Example", user_id: "AQ", user_name: "ana", challenge: "Ag", algorithms: [-8, -7], exclude: ["Aw"] });
    expect(getOptions({ challenge: bytes(4), rpId: "example.com", allowCredentials: [{ type: "public-key", id: bytes(5) }] }, "x")).toEqual({
      rp_id: "example.com",
      challenge: "BA",
      allow: ["BQ"],
    });
    expect(getOptions({ challenge: bytes(4) }, "host.test")).toEqual({ rp_id: "host.test", challenge: "BA", allow: [] });
  });

  it("builds credentials pages can read, with the browser's prototypes", () => {
    class FakeCredential {}
    class FakeAttestation {}
    class FakeAssertion {}
    const globals = { PublicKeyCredential: FakeCredential, AuthenticatorAttestationResponse: FakeAttestation, AuthenticatorAssertionResponse: FakeAssertion };
    const reg = registrationCredential(
      { credential_id: "AQ", client_data_json: "Ag", attestation_object: "Aw", authenticator_data: "BA", public_key: "BQ", public_key_algorithm: -7 },
      globals,
    );
    expect(reg).toBeInstanceOf(FakeCredential);
    expect(reg.response).toBeInstanceOf(FakeAttestation);
    expect(reg.id).toBe("AQ");
    expect([...new Uint8Array(reg.rawId)]).toEqual([1]);
    const attestation = reg.response as unknown as Record<string, () => unknown>;
    expect([...new Uint8Array(attestation.getPublicKey!() as ArrayBuffer)]).toEqual([5]);
    expect(attestation.getPublicKeyAlgorithm!()).toBe(-7);
    expect((reg.toJSON() as { response: { attestationObject: string } }).response.attestationObject).toBe("Aw");
    expect(reg.getClientExtensionResults()).toEqual({});

    const signIn = assertionCredential({ credential_id: "AQ", client_data_json: "Ag", authenticator_data: "Aw", signature: "BA", user_handle: "BQ" }, globals);
    expect(signIn.response).toBeInstanceOf(FakeAssertion);
    const assertion = signIn.response as AuthenticatorAssertionResponse;
    expect([...new Uint8Array(assertion.signature)]).toEqual([4]);
    expect([...new Uint8Array(assertion.userHandle!)]).toEqual([5]);
    expect(Object.getPrototypeOf(assertionCredential({ credential_id: "AQ", client_data_json: "", authenticator_data: "", signature: "", user_handle: "" }, {}))).toBe(Object.prototype);
  });
});
