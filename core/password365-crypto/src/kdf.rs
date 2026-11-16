//! Argon2id password-based key derivation with enforced security floors.

use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use crate::key::{KEY_LEN, SymmetricKey};
use crate::secret::SecretString;
use crate::{CryptoError, Result};

/// Minimum accepted salt length in bytes.
pub const MIN_SALT_LEN: usize = 16;

/// Argon2id cost parameters. Stored alongside the vault so they can be raised over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub memory_kib: u32,
    /// Number of passes.
    pub iterations: u32,
    /// Degree of parallelism.
    pub parallelism: u32,
}

impl KdfParams {
    /// Security floor: parameters read from disk below this are refused, so a
    /// tampered vault header cannot silently downgrade protection.
    pub const MINIMUM: Self = Self {
        memory_kib: 64 * 1024,
        iterations: 3,
        parallelism: 1,
    };

    /// Default for desktop-class devices (256 MiB, 3 passes, 4 lanes).
    pub const DESKTOP: Self = Self {
        memory_kib: 256 * 1024,
        iterations: 3,
        parallelism: 4,
    };

    /// Default for mobile devices (128 MiB, 4 passes, 4 lanes).
    pub const MOBILE: Self = Self {
        memory_kib: 128 * 1024,
        iterations: 4,
        parallelism: 4,
    };

    /// Tiny parameters that make tests fast. Only compiled for tests.
    #[cfg(any(test, feature = "insecure-test-params"))]
    #[must_use]
    pub const fn insecure_for_tests() -> Self {
        Self {
            memory_kib: 64,
            iterations: 1,
            parallelism: 1,
        }
    }

    /// Checks these parameters against the security floor.
    pub fn validate(&self) -> Result<()> {
        if self.parallelism == 0 || self.parallelism > 16 {
            return Err(CryptoError::WeakKdfParams("parallelism must be 1..=16"));
        }
        #[cfg(any(test, feature = "insecure-test-params"))]
        if *self == Self::insecure_for_tests() {
            return Ok(());
        }
        if self.memory_kib < Self::MINIMUM.memory_kib {
            return Err(CryptoError::WeakKdfParams("memory below 64 MiB"));
        }
        if self.iterations < Self::MINIMUM.iterations {
            return Err(CryptoError::WeakKdfParams("fewer than 3 iterations"));
        }
        Ok(())
    }
}

impl Default for KdfParams {
    fn default() -> Self {
        Self::DESKTOP
    }
}

/// Normalizes a password to Unicode NFKD so the same password typed on
/// different keyboards / OSes derives the same key.
#[must_use]
pub fn normalize_password(password: &SecretString) -> Zeroizing<String> {
    Zeroizing::new(password.expose().nfkd().collect())
}

/// Derives a 256-bit key from a password with Argon2id.
pub fn derive_password_key(
    password: &SecretString,
    salt: &[u8],
    params: KdfParams,
) -> Result<SymmetricKey> {
    params.validate()?;
    if salt.len() < MIN_SALT_LEN {
        return Err(CryptoError::Malformed("salt shorter than 16 bytes"));
    }
    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|_| CryptoError::WeakKdfParams("rejected by argon2"))?;
    let normalized = normalize_password(password);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params)
        .hash_password_into(normalized.as_bytes(), salt, out.as_mut())
        .map_err(|_| CryptoError::Kdf)?;
    Ok(SymmetricKey::from_bytes(*out))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALT: [u8; 16] = [9; 16];

    fn fast() -> KdfParams {
        KdfParams::insecure_for_tests()
    }

    #[test]
    fn presets_pass_validation() {
        KdfParams::DESKTOP.validate().unwrap();
        KdfParams::MOBILE.validate().unwrap();
        KdfParams::MINIMUM.validate().unwrap();
        assert_eq!(KdfParams::default(), KdfParams::DESKTOP);
    }

    #[test]
    fn low_memory_is_rejected() {
        let p = KdfParams {
            memory_kib: 1024,
            ..KdfParams::DESKTOP
        };
        assert!(matches!(p.validate(), Err(CryptoError::WeakKdfParams(_))));
    }

    #[test]
    fn low_iterations_are_rejected() {
        let p = KdfParams {
            iterations: 1,
            ..KdfParams::DESKTOP
        };
        assert!(matches!(p.validate(), Err(CryptoError::WeakKdfParams(_))));
    }

    #[test]
    fn bad_parallelism_is_rejected() {
        for parallelism in [0, 17] {
            let p = KdfParams {
                parallelism,
                ..KdfParams::DESKTOP
            };
            assert!(matches!(p.validate(), Err(CryptoError::WeakKdfParams(_))));
        }
    }

    #[test]
    fn short_salt_is_rejected() {
        let r = derive_password_key(&"pw".into(), &[0; 8], fast());
        assert!(matches!(r, Err(CryptoError::Malformed(_))));
    }

    #[test]
    fn weak_params_are_rejected_before_hashing() {
        let weak = KdfParams {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        assert!(derive_password_key(&"pw".into(), &SALT, weak).is_err());
    }

    #[test]
    fn same_inputs_same_key() {
        let a = derive_password_key(&"correct horse".into(), &SALT, fast()).unwrap();
        let b = derive_password_key(&"correct horse".into(), &SALT, fast()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn different_password_different_key() {
        let a = derive_password_key(&"a".into(), &SALT, fast()).unwrap();
        let b = derive_password_key(&"b".into(), &SALT, fast()).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn different_salt_different_key() {
        let a = derive_password_key(&"a".into(), &[1; 16], fast()).unwrap();
        let b = derive_password_key(&"a".into(), &[2; 16], fast()).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn composed_and_decomposed_unicode_derive_same_key() {
        let composed = "caf\u{e9}"; // é as one code point
        let decomposed = "cafe\u{301}"; // e + combining acute
        let a = derive_password_key(&composed.into(), &SALT, fast()).unwrap();
        let b = derive_password_key(&decomposed.into(), &SALT, fast()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn normalize_is_nfkd() {
        assert_eq!(normalize_password(&"\u{e9}".into()).as_str(), "e\u{301}");
    }

    #[test]
    fn params_serialize_round_trip() {
        let json = serde_json::to_string(&KdfParams::MOBILE).unwrap();
        let back: KdfParams = serde_json::from_str(&json).unwrap();
        assert_eq!(back, KdfParams::MOBILE);
    }
}
