//! Key wrapping: encrypting one symmetric key under another.

use crate::Result;
use crate::aad::Aad;
use crate::aead;
use crate::key::SymmetricKey;

/// Encrypts `key` under the key-encryption key `kek`, bound to `aad`.
pub fn wrap_key(kek: &SymmetricKey, key: &SymmetricKey, aad: &Aad) -> Result<Vec<u8>> {
    aead::seal(kek, key.expose(), aad)
}

/// Decrypts a key produced by [`wrap_key`].
pub fn unwrap_key(kek: &SymmetricKey, wrapped: &[u8], aad: &Aad) -> Result<SymmetricKey> {
    let plaintext = aead::open(kek, wrapped, aad)?;
    SymmetricKey::from_slice(&plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CryptoError;

    #[test]
    fn wrap_unwrap_round_trip() {
        let kek = SymmetricKey::generate().unwrap();
        let key = SymmetricKey::generate().unwrap();
        let aad = Aad::new("wrap");
        let wrapped = wrap_key(&kek, &key, &aad).unwrap();
        assert_eq!(unwrap_key(&kek, &wrapped, &aad).unwrap(), key);
    }

    #[test]
    fn wrong_kek_fails() {
        let key = SymmetricKey::generate().unwrap();
        let wrapped = wrap_key(&SymmetricKey::generate().unwrap(), &key, &Aad::new("w")).unwrap();
        let result = unwrap_key(&SymmetricKey::generate().unwrap(), &wrapped, &Aad::new("w"));
        assert_eq!(result, Err(CryptoError::Decrypt));
    }

    #[test]
    fn wrong_length_payload_is_rejected() {
        let kek = SymmetricKey::generate().unwrap();
        let aad = Aad::new("w");
        let not_a_key = aead::seal(&kek, b"short", &aad).unwrap();
        assert!(matches!(
            unwrap_key(&kek, &not_a_key, &aad),
            Err(CryptoError::Malformed(_))
        ));
    }
}
