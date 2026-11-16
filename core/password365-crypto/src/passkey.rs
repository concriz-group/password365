//! Passkey keys: ECDSA P-256 (COSE algorithm -7, "ES256"), the algorithm every
//! site accepts. Signatures are deterministic (RFC 6979), so signing needs no
//! randomness.

use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use zeroize::Zeroizing;

use crate::{CryptoError, Result, random};

/// COSE algorithm id of ES256.
pub const ES256: i64 = -7;

/// A passkey's private key.
pub struct PasskeyKey(SigningKey);

impl std::fmt::Debug for PasskeyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PasskeyKey(..)")
    }
}

impl PasskeyKey {
    /// A new random key.
    pub fn generate() -> Result<Self> {
        loop {
            let bytes = Zeroizing::new(random::bytes::<32>()?);
            // A random 32-byte value is a valid scalar except with
            // negligible probability; draw again in that case.
            if let Ok(key) = SigningKey::from_slice(bytes.as_slice()) {
                return Ok(Self(key));
            }
        }
    }

    /// The key from its 32 secret bytes.
    pub fn from_secret_bytes(bytes: &[u8]) -> Result<Self> {
        SigningKey::from_slice(bytes)
            .map(Self)
            .map_err(|_| CryptoError::Malformed("passkey private key"))
    }

    /// The key from PKCS#8 DER (a credential-exchange file). Only P-256
    /// keys are accepted.
    pub fn from_pkcs8_der(der: &[u8]) -> Result<Self> {
        use p256::pkcs8::DecodePrivateKey;
        SigningKey::from_pkcs8_der(der)
            .map(Self)
            .map_err(|_| CryptoError::Malformed("passkey private key (PKCS#8 P-256)"))
    }

    /// The 32 secret bytes (to store inside the encrypted item).
    #[must_use]
    pub fn secret_bytes(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.0.to_bytes().to_vec())
    }

    /// The public point's x and y coordinates.
    #[must_use]
    pub fn public_xy(&self) -> ([u8; 32], [u8; 32]) {
        let point = self.0.verifying_key().to_sec1_point(false);
        let bytes = point.as_bytes();
        let mut x = [0; 32];
        let mut y = [0; 32];
        x.copy_from_slice(&bytes[1..33]);
        y.copy_from_slice(&bytes[33..65]);
        (x, y)
    }

    /// The key as PKCS#8 DER (RFC 5208 / RFC 5915, with the public key), the
    /// form credential-exchange files carry.
    #[must_use]
    pub fn pkcs8_der(&self) -> Zeroizing<Vec<u8>> {
        // PrivateKeyInfo { version 0, ecPublicKey/prime256v1, ECPrivateKey }.
        const HEAD: [u8; 36] = [
            0x30, 0x81, 0x87, 0x02, 0x01, 0x00, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce,
            0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x04,
            0x6d, 0x30, 0x6b, 0x02, 0x01, 0x01, 0x04, 0x20,
        ];
        let (x, y) = self.public_xy();
        let mut out = Zeroizing::new(Vec::with_capacity(138));
        out.extend_from_slice(&HEAD);
        out.extend_from_slice(&self.secret_bytes());
        out.extend_from_slice(&[0xa1, 0x44, 0x03, 0x42, 0x00, 0x04]);
        out.extend_from_slice(&x);
        out.extend_from_slice(&y);
        out
    }

    /// An ASN.1 DER signature of `message` (what `WebAuthn` expects).
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let signature: Signature = self.0.sign(message);
        signature.to_der().to_bytes().to_vec()
    }
}

/// Checks a DER signature against public coordinates (tests and self-checks).
#[must_use]
pub fn verify(x: &[u8; 32], y: &[u8; 32], message: &[u8], der: &[u8]) -> bool {
    let mut sec1 = [0u8; 65];
    sec1[0] = 4;
    sec1[1..33].copy_from_slice(x);
    sec1[33..].copy_from_slice(y);
    let (Ok(key), Ok(signature)) = (
        VerifyingKey::from_sec1_bytes(&sec1),
        Signature::from_der(der),
    ) else {
        return false;
    };
    key.verify(message, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_and_round_trips_through_its_secret_bytes() {
        let key = PasskeyKey::generate().unwrap();
        let (x, y) = key.public_xy();
        let der = key.sign(b"auth data and client hash");
        assert!(verify(&x, &y, b"auth data and client hash", &der));
        assert!(!verify(&x, &y, b"something else", &der));
        assert!(!verify(&x, &y, b"x", b"not der"));

        let again = PasskeyKey::from_secret_bytes(&key.secret_bytes()).unwrap();
        assert_eq!(again.public_xy(), (x, y));
        assert_eq!(again.sign(b"m"), key.sign(b"m"), "deterministic");
        assert_ne!(PasskeyKey::generate().unwrap().public_xy(), (x, y));
        assert!(PasskeyKey::from_secret_bytes(&[0; 32]).is_err());
        assert!(PasskeyKey::from_secret_bytes(&[1; 5]).is_err());
        assert_eq!(format!("{key:?}"), "PasskeyKey(..)");
        let der = key.pkcs8_der();
        assert_eq!(der.len(), 138);
        assert_eq!(&der[36..68], key.secret_bytes().as_slice());
        assert_eq!(&der[74..106], &x);
        let back = PasskeyKey::from_pkcs8_der(&der).unwrap();
        assert_eq!(back.public_xy(), (x, y), "PKCS#8 round trip");
        assert!(PasskeyKey::from_pkcs8_der(&der[..50]).is_err());
        assert!(PasskeyKey::from_pkcs8_der(b"not der").is_err());
    }

    #[test]
    fn matches_an_independent_test_vector() {
        // RFC 6979 A.2.5: P-256, private key x, message "sample", SHA-256.
        let secret =
            hex::decode("C9AFA9D845BA75166B5C215767B1D6934E50C3DB36E89B127B8A622B120F6721")
                .unwrap();
        let key = PasskeyKey::from_secret_bytes(&secret).unwrap();
        let (x, y) = key.public_xy();
        assert_eq!(
            hex::encode_upper(x),
            "60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6"
        );
        assert_eq!(
            hex::encode_upper(y),
            "7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299"
        );
        let signature = Signature::from_der(&key.sign(b"sample"))
            .unwrap()
            .to_bytes();
        assert_eq!(
            hex::encode_upper(signature),
            "EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716\
             F7CB1C942D657C41D436C7A1B6E29F65F3E900DBB9AFF4064DC4AB2F843ACDA8"
        );
    }
}
