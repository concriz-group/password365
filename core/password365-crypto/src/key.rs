//! 256-bit symmetric key.

use std::fmt;

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{CryptoError, Result, random};

/// Length in bytes of every symmetric key in Password365.
pub const KEY_LEN: usize = 32;

/// A 256-bit symmetric key. Zeroized when dropped; never printed.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SymmetricKey([u8; KEY_LEN]);

impl SymmetricKey {
    /// Generates a fresh random key from the OS CSPRNG.
    pub fn generate() -> Result<Self> {
        random::bytes().map(Self)
    }

    /// Builds a key from raw bytes (takes ownership so the caller's copy is not kept).
    #[must_use]
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Builds a key from a slice, checking its length.
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let array: [u8; KEY_LEN] = bytes
            .try_into()
            .map_err(|_| CryptoError::Malformed("key must be 32 bytes"))?;
        Ok(Self(array))
    }

    /// Exposes the raw key bytes. Keep the borrow as short as possible.
    #[must_use]
    pub fn expose(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl PartialEq for SymmetricKey {
    /// Constant-time comparison.
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for SymmetricKey {}

impl fmt::Debug for SymmetricKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SymmetricKey(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_keys_differ() {
        assert_ne!(
            SymmetricKey::generate().unwrap(),
            SymmetricKey::generate().unwrap()
        );
    }

    #[test]
    fn from_bytes_round_trips() {
        let key = SymmetricKey::from_bytes([7; KEY_LEN]);
        assert_eq!(key.expose(), &[7; KEY_LEN]);
    }

    #[test]
    fn from_slice_accepts_exact_length() {
        let key = SymmetricKey::from_slice(&[1; KEY_LEN]).unwrap();
        assert_eq!(key, SymmetricKey::from_bytes([1; KEY_LEN]));
    }

    #[test]
    fn from_slice_rejects_wrong_length() {
        assert!(SymmetricKey::from_slice(&[1; 31]).is_err());
        assert!(SymmetricKey::from_slice(&[1; 33]).is_err());
    }

    #[test]
    fn debug_never_prints_key_material() {
        let key = SymmetricKey::from_bytes([0xAB; KEY_LEN]);
        let printed = format!("{key:?}");
        assert_eq!(printed, "SymmetricKey(<redacted>)");
        assert!(!printed.contains("171"));
    }

    #[test]
    fn zeroize_clears_bytes() {
        let mut key = SymmetricKey::from_bytes([5; KEY_LEN]);
        key.zeroize();
        assert_eq!(key.expose(), &[0; KEY_LEN]);
    }

    #[test]
    fn clone_is_equal() {
        let key = SymmetricKey::generate().unwrap();
        assert_eq!(key.clone(), key);
    }
}
