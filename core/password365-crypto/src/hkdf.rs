//! HKDF-SHA-256 sub-key derivation (RFC 5869).

use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::key::{KEY_LEN, SymmetricKey};
use crate::{CryptoError, Result};

/// Derives a 256-bit key from input keying material with a salt and a
/// context-specific `info` label.
pub fn derive_key(ikm: &[u8], salt: &[u8], info: &[u8]) -> Result<SymmetricKey> {
    let mut okm = Zeroizing::new([0u8; KEY_LEN]);
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, okm.as_mut())
        .map_err(|_| CryptoError::Kdf)?;
    Ok(SymmetricKey::from_bytes(*okm))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc5869_test_case_1_prefix() {
        // RFC 5869 A.1 (first 32 bytes of the 42-byte OKM).
        let ikm = [0x0b; 22];
        let salt = hex::decode("000102030405060708090a0b0c").unwrap();
        let info = hex::decode("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let key = derive_key(&ikm, &salt, &info).unwrap();
        assert_eq!(
            hex::encode(key.expose()),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"
        );
    }

    #[test]
    fn different_info_gives_different_keys() {
        let a = derive_key(b"ikm", b"salt", b"a").unwrap();
        let b = derive_key(b"ikm", b"salt", b"b").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn different_salt_gives_different_keys() {
        let a = derive_key(b"ikm", b"salt1", b"i").unwrap();
        let b = derive_key(b"ikm", b"salt2", b"i").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn deterministic() {
        assert_eq!(
            derive_key(b"ikm", b"salt", b"i").unwrap(),
            derive_key(b"ikm", b"salt", b"i").unwrap()
        );
    }
}
