//! Creating passkeys and signing in with them.

use password365_crypto::passkey::{ES256, PasskeyKey};
use password365_crypto::random;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::cbor::Cbor;
use crate::{Result, WebAuthnError, b64url, from_b64url, rp};

/// Flags in authenticator data.
const USER_PRESENT: u8 = 0x01;
const USER_VERIFIED: u8 = 0x04;
const BACKUP_ELIGIBLE: u8 = 0x08;
const BACKED_UP: u8 = 0x10;
const ATTESTED_DATA: u8 = 0x40;
/// Authenticator model id. All zeros: attestation is "none", so the model is
/// not claimed.
const AAGUID: [u8; 16] = [0; 16];
/// DER prefix of a P-256 `SubjectPublicKeyInfo` (followed by `04 || x || y`).
const SPKI_P256_PREFIX: [u8; 26] = [
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];

/// A stored passkey (kept inside an encrypted login item).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Passkey {
    /// Credential id (base64url).
    pub credential_id: String,
    /// Relying-party id, e.g. `example.com`.
    pub rp_id: String,
    /// The site's id for the account (base64url), returned at sign-in.
    pub user_handle: String,
    /// Account name the site gave (e.g. an e-mail address).
    pub user_name: String,
    /// The private key's 32 bytes (base64url).
    pub key: String,
}

impl std::fmt::Debug for Passkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Passkey")
            .field("credential_id", &self.credential_id)
            .field("rp_id", &self.rp_id)
            .field("user_name", &self.user_name)
            .finish_non_exhaustive()
    }
}

/// The page's `userVerification` wish (absent means "preferred").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UserVerification {
    /// The sign-in must fail without a verified user.
    Required,
    /// Verify when possible; the site accepts an unverified answer.
    #[default]
    Preferred,
    /// The site does not want verification.
    Discouraged,
}

/// What `navigator.credentials.create` asks for (the parts used here).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateRequest {
    /// Relying-party id (defaults to the page's host when the page omits it).
    pub rp_id: String,
    /// The site's id for the account (base64url).
    pub user_id: String,
    /// Account name.
    pub user_name: String,
    /// Challenge (base64url).
    pub challenge: String,
    /// COSE algorithms the site accepts, in its order of preference.
    pub algorithms: Vec<i64>,
    /// Whether the site wants the user verified.
    #[serde(default)]
    pub user_verification: UserVerification,
}

/// The answer to `create`, ready for the page (binary values base64url).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registration {
    /// Credential id.
    pub credential_id: String,
    /// `clientDataJSON`.
    pub client_data_json: String,
    /// `attestationObject`.
    pub attestation_object: String,
    /// `getAuthenticatorData()`.
    pub authenticator_data: String,
    /// `getPublicKey()`: DER `SubjectPublicKeyInfo`.
    pub public_key: String,
    /// `getPublicKeyAlgorithm()`.
    pub public_key_algorithm: i64,
}

/// The answer to `get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assertion {
    /// Credential id.
    pub credential_id: String,
    /// `clientDataJSON`.
    pub client_data_json: String,
    /// `authenticatorData`.
    pub authenticator_data: String,
    /// DER ECDSA signature over `authenticatorData || SHA-256(clientDataJSON)`.
    pub signature: String,
    /// The account's id at the site.
    pub user_handle: String,
}

/// `clientDataJSON` for `kind` ("webauthn.create" / "webauthn.get"),
/// members in the order browsers write them.
fn client_data(kind: &str, challenge: &[u8], origin: &str) -> Vec<u8> {
    let text = |s: &str| serde_json::to_string(s).unwrap_or_default();
    format!(
        r#"{{"type":{},"challenge":{},"origin":{},"crossOrigin":false}}"#,
        text(kind),
        text(&b64url(challenge)),
        text(origin)
    )
    .into_bytes()
}

/// The flags of every answer: the user is present, and verified only when
/// `verified` (the master password or biometrics proved it was them).
const fn flags(verified: bool) -> u8 {
    let base = USER_PRESENT | BACKUP_ELIGIBLE | BACKED_UP;
    if verified { base | USER_VERIFIED } else { base }
}

fn auth_data(rp_id: &str, flags: u8, attested: Option<&[u8]>) -> Vec<u8> {
    let mut out = Sha256::digest(rp_id.as_bytes()).to_vec();
    out.push(flags);
    out.extend_from_slice(&0u32.to_be_bytes());
    if let Some(data) = attested {
        out.extend_from_slice(data);
    }
    out
}

fn cose_key(x: &[u8; 32], y: &[u8; 32]) -> Vec<u8> {
    Cbor::Map(vec![
        (Cbor::Int(1), Cbor::Int(2)),     // kty: EC2
        (Cbor::Int(3), Cbor::Int(ES256)), // alg
        (Cbor::Int(-1), Cbor::Int(1)),    // crv: P-256
        (Cbor::Int(-2), Cbor::Bytes(x.to_vec())),
        (Cbor::Int(-3), Cbor::Bytes(y.to_vec())),
    ])
    .encode()
}

/// Creates a passkey for `page_url` (the tab's real address).
/// `verified`: the user proved who they are (master password or biometrics);
/// only then does the answer say so.
pub fn register(
    page_url: &str,
    request: &CreateRequest,
    verified: bool,
) -> Result<(Passkey, Registration)> {
    let origin = rp::check(page_url, &request.rp_id)?;
    if !request.algorithms.is_empty() && !request.algorithms.contains(&ES256) {
        return Err(WebAuthnError::UnsupportedAlgorithm);
    }
    let challenge = from_b64url(&request.challenge)?;
    let user_handle = from_b64url(&request.user_id)?;
    if user_handle.is_empty() || user_handle.len() > 64 {
        return Err(WebAuthnError::Invalid("user id"));
    }
    let rp_id = request.rp_id.trim().to_ascii_lowercase();
    let made = new_credential(&rp_id, &user_handle, &request.user_name, verified)?;
    let registration = Registration {
        credential_id: made.passkey.credential_id.clone(),
        client_data_json: b64url(&client_data("webauthn.create", &challenge, &origin)),
        attestation_object: b64url(&made.attestation),
        authenticator_data: b64url(&made.auth_data),
        public_key: b64url(&made.spki),
        public_key_algorithm: ES256,
    };
    Ok((made.passkey, registration))
}

/// A new key pair and what registration returns about it.
struct NewCredential {
    passkey: Passkey,
    attestation: Vec<u8>,
    auth_data: Vec<u8>,
    spki: Vec<u8>,
}

fn new_credential(
    rp_id: &str,
    user_handle: &[u8],
    user_name: &str,
    verified: bool,
) -> Result<NewCredential> {
    let key = PasskeyKey::generate()?;
    let id = random::bytes::<16>()?;
    let (x, y) = key.public_xy();

    let mut attested = AAGUID.to_vec();
    attested.extend_from_slice(&u16::try_from(id.len()).unwrap_or(16).to_be_bytes());
    attested.extend_from_slice(&id);
    attested.extend_from_slice(&cose_key(&x, &y));
    let data = auth_data(rp_id, flags(verified) | ATTESTED_DATA, Some(&attested));
    let attestation = Cbor::Map(vec![
        (Cbor::Text("fmt".into()), Cbor::Text("none".into())),
        (Cbor::Text("attStmt".into()), Cbor::Map(vec![])),
        (Cbor::Text("authData".into()), Cbor::Bytes(data.clone())),
    ])
    .encode();
    let mut spki = SPKI_P256_PREFIX.to_vec();
    spki.push(4);
    spki.extend_from_slice(&x);
    spki.extend_from_slice(&y);

    let passkey = Passkey {
        credential_id: b64url(&id),
        rp_id: rp_id.to_owned(),
        user_handle: b64url(user_handle),
        user_name: user_name.to_owned(),
        key: b64url(&key.secret_bytes()),
    };
    Ok(NewCredential {
        passkey,
        attestation,
        auth_data: data,
        spki,
    })
}

/// Authenticator data and signature over it and `client_data_hash`.
fn sign(passkey: &Passkey, client_data_hash: &[u8], verified: bool) -> Result<(Vec<u8>, Vec<u8>)> {
    let secret = Zeroizing::new(from_b64url(&passkey.key)?);
    let key = PasskeyKey::from_secret_bytes(&secret)?;
    let data = auth_data(&passkey.rp_id, flags(verified), None);
    let mut signed = data.clone();
    signed.extend_from_slice(client_data_hash);
    let signature = key.sign(&signed);
    Ok((data, signature))
}

/// What the phone's `AutoFill` hands back after creating a passkey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformRegistration {
    /// Credential id (raw bytes).
    pub credential_id: Vec<u8>,
    /// `attestationObject` (CBOR).
    pub attestation_object: Vec<u8>,
}

/// What the phone's `AutoFill` hands back when signing in with a passkey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformAssertion {
    /// Credential id (raw bytes).
    pub credential_id: Vec<u8>,
    /// `authenticatorData`.
    pub authenticator_data: Vec<u8>,
    /// DER ECDSA signature.
    pub signature: Vec<u8>,
    /// The account's id at the site (raw bytes).
    pub user_handle: Vec<u8>,
}

/// Creates a passkey for the operating system's `AutoFill` (iOS credential
/// provider): the system has already checked that the app or site may use
/// `rp_id` and built `clientDataJSON`; it gives only its SHA-256.
pub fn register_for_platform(
    rp_id: &str,
    user_handle: &[u8],
    user_name: &str,
    algorithms: &[i64],
    verified: bool,
) -> Result<(Passkey, PlatformRegistration)> {
    if !algorithms.is_empty() && !algorithms.contains(&ES256) {
        return Err(WebAuthnError::UnsupportedAlgorithm);
    }
    if user_handle.is_empty() || user_handle.len() > 64 {
        return Err(WebAuthnError::Invalid("user id"));
    }
    let made = new_credential(
        &rp_id.trim().to_ascii_lowercase(),
        user_handle,
        user_name,
        verified,
    )?;
    let credential_id = from_b64url(&made.passkey.credential_id)?;
    Ok((
        made.passkey,
        PlatformRegistration {
            credential_id,
            attestation_object: made.attestation,
        },
    ))
}

/// Signs in with `passkey` for the operating system's `AutoFill`, over the
/// SHA-256 of the `clientDataJSON` the system built.
pub fn assert_for_platform(
    passkey: &Passkey,
    client_data_hash: &[u8],
    verified: bool,
) -> Result<PlatformAssertion> {
    if client_data_hash.len() != 32 {
        return Err(WebAuthnError::Invalid("client data hash"));
    }
    let (authenticator_data, signature) = sign(passkey, client_data_hash, verified)?;
    Ok(PlatformAssertion {
        credential_id: from_b64url(&passkey.credential_id)?,
        authenticator_data,
        signature,
        user_handle: from_b64url(&passkey.user_handle)?,
    })
}

/// Signs in to `page_url` with `passkey`, answering `challenge` (base64url).
pub fn assert(
    page_url: &str,
    passkey: &Passkey,
    challenge: &str,
    verified: bool,
) -> Result<Assertion> {
    let origin = rp::check(page_url, &passkey.rp_id)?;
    let challenge = from_b64url(challenge)?;
    let client = client_data("webauthn.get", &challenge, &origin);
    let (data, signature) = sign(passkey, &Sha256::digest(&client), verified)?;
    Ok(Assertion {
        credential_id: passkey.credential_id.clone(),
        client_data_json: b64url(&client),
        authenticator_data: b64url(&data),
        signature: b64url(&signature),
        user_handle: passkey.user_handle.clone(),
    })
}

#[cfg(test)]
mod tests {
    use password365_crypto::passkey::verify;

    use super::*;

    fn request() -> CreateRequest {
        CreateRequest {
            rp_id: "example.com".into(),
            user_id: b64url(b"user-42"),
            user_name: "ana@example.com".into(),
            challenge: b64url(b"challenge-1"),
            algorithms: vec![-8, -7, -257],
            user_verification: UserVerification::Preferred,
        }
    }

    #[test]
    fn user_verified_is_claimed_only_when_it_happened() {
        let (passkey, reg) = register("https://example.com", &request(), false).unwrap();
        let attestation = from_b64url(&reg.attestation_object).unwrap();
        let pos = attestation
            .windows(32)
            .position(|w| w == Sha256::digest(b"example.com").as_slice())
            .unwrap();
        assert_eq!(attestation[pos + 32] & USER_VERIFIED, 0);
        let answer = assert("https://example.com", &passkey, "AA", false).unwrap();
        assert_eq!(
            from_b64url(&answer.authenticator_data).unwrap()[32] & USER_VERIFIED,
            0
        );
        let answer = assert("https://example.com", &passkey, "AA", true).unwrap();
        assert_eq!(
            from_b64url(&answer.authenticator_data).unwrap()[32] & USER_VERIFIED,
            USER_VERIFIED
        );
        let hash = [1u8; 32];
        assert_eq!(
            assert_for_platform(&passkey, &hash, false)
                .unwrap()
                .authenticator_data[32]
                & USER_VERIFIED,
            0
        );
    }

    #[test]
    fn registration_has_the_shape_relying_parties_verify() {
        let (passkey, reg) =
            register("https://login.example.com/signup", &request(), true).unwrap();
        assert_eq!(
            (passkey.rp_id.as_str(), passkey.user_name.as_str()),
            ("example.com", "ana@example.com")
        );
        let client: serde_json::Value =
            serde_json::from_slice(&from_b64url(&reg.client_data_json).unwrap()).unwrap();
        assert_eq!(client["type"], "webauthn.create");
        assert_eq!(client["challenge"], b64url(b"challenge-1"));
        assert_eq!(client["origin"], "https://login.example.com");
        assert!(
            String::from_utf8(from_b64url(&reg.client_data_json).unwrap())
                .unwrap()
                .starts_with(r#"{"type":"#)
        );

        let data = from_b64url(&reg.authenticator_data).unwrap();
        assert_eq!(&data[..32], Sha256::digest(b"example.com").as_slice());
        assert_eq!(data[32], 0x5d, "UP, UV, BE, BS, AT");
        assert_eq!(&data[33..37], &[0, 0, 0, 0]);
        assert_eq!(&data[37..53], &AAGUID);
        let id = from_b64url(&reg.credential_id).unwrap();
        assert_eq!(&data[53..55], &[0, 16]);
        assert_eq!(&data[55..71], id.as_slice());
        let key = PasskeyKey::from_secret_bytes(&from_b64url(&passkey.key).unwrap()).unwrap();
        let (x, y) = key.public_xy();
        assert_eq!(&data[71..], cose_key(&x, &y).as_slice());

        let attestation = from_b64url(&reg.attestation_object).unwrap();
        assert!(attestation.starts_with(
            &hex::decode("a363666d74646e6f6e656761747453746d74a068617574684461746158").unwrap()
        ));
        assert!(attestation.ends_with(&data));
        let spki = from_b64url(&reg.public_key).unwrap();
        assert_eq!(
            (spki.len(), &spki[26..27], &spki[27..59]),
            (91, &[4u8][..], &x[..])
        );
        assert_eq!(reg.public_key_algorithm, -7);
    }

    #[test]
    fn sign_in_answers_are_signed_by_the_stored_key() {
        let (passkey, reg) = register("https://example.com", &request(), true).unwrap();
        let answer = assert(
            "https://www.example.com/login",
            &passkey,
            &b64url(b"challenge-2"),
            true,
        )
        .unwrap();
        assert_eq!(
            (answer.credential_id.as_str(), answer.user_handle.as_str()),
            (reg.credential_id.as_str(), b64url(b"user-42").as_str())
        );
        let data = from_b64url(&answer.authenticator_data).unwrap();
        assert_eq!((data.len(), data[32]), (37, 0x1d));
        let client = from_b64url(&answer.client_data_json).unwrap();
        let mut signed = data;
        signed.extend_from_slice(&Sha256::digest(&client));
        let key = PasskeyKey::from_secret_bytes(&from_b64url(&passkey.key).unwrap()).unwrap();
        let (x, y) = key.public_xy();
        assert!(verify(
            &x,
            &y,
            &signed,
            &from_b64url(&answer.signature).unwrap()
        ));
        let client: serde_json::Value = serde_json::from_slice(&client).unwrap();
        assert_eq!(
            (client["type"].as_str(), client["origin"].as_str()),
            (Some("webauthn.get"), Some("https://www.example.com"))
        );
    }

    #[test]
    fn platform_autofill_signs_over_the_systems_client_data_hash() {
        let (passkey, reg) =
            register_for_platform("Example.com", b"user-7", "ana", &[-7], true).unwrap();
        assert_eq!(passkey.rp_id, "example.com");
        assert_eq!(
            reg.credential_id,
            from_b64url(&passkey.credential_id).unwrap()
        );
        assert!(
            reg.attestation_object
                .starts_with(&hex::decode("a363666d74646e6f6e65").unwrap())
        );
        let hash = Sha256::digest(b"{\"type\":\"webauthn.get\"}");
        let answer = assert_for_platform(&passkey, &hash, true).unwrap();
        assert_eq!(
            (
                answer.user_handle.as_slice(),
                answer.authenticator_data.len()
            ),
            (b"user-7".as_slice(), 37)
        );
        let key = PasskeyKey::from_secret_bytes(&from_b64url(&passkey.key).unwrap()).unwrap();
        let (x, y) = key.public_xy();
        let mut signed = answer.authenticator_data.clone();
        signed.extend_from_slice(&hash);
        assert!(verify(&x, &y, &signed, &answer.signature));
        assert!(assert_for_platform(&passkey, b"short", true).is_err());
        assert_eq!(
            register_for_platform("a.com", b"u", "", &[-257], true).unwrap_err(),
            WebAuthnError::UnsupportedAlgorithm
        );
        assert!(register_for_platform("a.com", b"", "", &[], true).is_err());
    }

    #[test]
    fn wrong_sites_algorithms_and_data_are_refused() {
        assert!(matches!(
            register("https://evil.com", &request(), true),
            Err(WebAuthnError::WrongSite(_))
        ));
        let rsa_only = CreateRequest {
            algorithms: vec![-257],
            ..request()
        };
        assert_eq!(
            register("https://example.com", &rsa_only, true).unwrap_err(),
            WebAuthnError::UnsupportedAlgorithm
        );
        let no_user = CreateRequest {
            user_id: String::new(),
            ..request()
        };
        assert!(register("https://example.com", &no_user, true).is_err());
        let bad = CreateRequest {
            challenge: "%%%".into(),
            ..request()
        };
        assert!(register("https://example.com", &bad, true).is_err());
        let (passkey, _) = register("https://example.com", &request(), true).unwrap();
        assert!(matches!(
            assert("https://evil.com", &passkey, "AA", true),
            Err(WebAuthnError::WrongSite(_))
        ));
        let broken = Passkey {
            key: "AA".into(),
            ..passkey.clone()
        };
        assert!(assert("https://example.com", &broken, "AA", true).is_err());
        assert!(
            !format!("{passkey:?}").contains(&passkey.key),
            "the key is never printed"
        );
        assert_eq!(from_b64url("YWI=").unwrap(), b"ab");
    }
}
