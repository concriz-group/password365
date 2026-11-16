//! Encode → pad → encrypt (and the reverse) for any serializable value.

use password365_crypto::aad::Aad;
use password365_crypto::aead;
use password365_crypto::key::SymmetricKey;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Result, codec, padding};

/// Encrypts a value under `key`, bound to `aad`.
pub(crate) fn seal<T: Serialize>(key: &SymmetricKey, value: &T, aad: &Aad) -> Result<Vec<u8>> {
    let encoded = codec::encode(value)?;
    let padded = padding::pad(&encoded)?;
    Ok(aead::seal(key, &padded, aad)?)
}

/// Decrypts a value produced by [`seal`].
pub(crate) fn open<T: DeserializeOwned>(key: &SymmetricKey, sealed: &[u8], aad: &Aad) -> Result<T> {
    let padded = aead::open(key, sealed, aad)?;
    codec::decode(padding::unpad(&padded)?)
}

/// Moves a sealed value from `old` to `new` without decoding it (so records
/// of kinds this build does not know survive a key change).
pub(crate) fn reseal(
    old: &SymmetricKey,
    new: &SymmetricKey,
    sealed: &[u8],
    aad: &Aad,
) -> Result<Vec<u8>> {
    let padded = zeroize::Zeroizing::new(aead::open(old, sealed, aad)?);
    Ok(aead::seal(new, &padded, aad)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VaultError;

    #[test]
    fn round_trip() {
        let key = SymmetricKey::generate().unwrap();
        let aad = Aad::new("t");
        let sealed = seal(&key, &("hello".to_string(), 42u32), &aad).unwrap();
        let (s, n): (String, u32) = open(&key, &sealed, &aad).unwrap();
        assert_eq!((s.as_str(), n), ("hello", 42));
    }

    #[test]
    fn ciphertext_size_is_bucketed() {
        let key = SymmetricKey::generate().unwrap();
        let aad = Aad::new("t");
        let a = seal(&key, &"a".to_string(), &aad).unwrap();
        let b = seal(
            &key,
            &"a much longer string but still small".to_string(),
            &aad,
        )
        .unwrap();
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn wrong_key_is_a_crypto_error() {
        let aad = Aad::new("t");
        let sealed = seal(&SymmetricKey::generate().unwrap(), &1u8, &aad).unwrap();
        let result: Result<u8> = open(&SymmetricKey::generate().unwrap(), &sealed, &aad);
        assert!(matches!(result, Err(VaultError::Crypto(_))));
    }
}
