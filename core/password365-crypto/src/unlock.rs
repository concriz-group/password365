//! Derivation of the Account Unlock Key (AUK) from the two secrets.
//!
//! ```text
//! k_pw = Argon2id(NFKD(password), salt, params)
//! k_sk = HKDF-SHA256(ikm = secret_key, salt = account_id, info = "quillock/v1/secret-key")
//! AUK  = HKDF-SHA256(ikm = k_pw || k_sk, salt = salt,       info = "quillock/v1/auk")
//! ```

use zeroize::Zeroizing;

use crate::Result;
use crate::hkdf::derive_key;
use crate::kdf::{KdfParams, derive_password_key};
use crate::key::{KEY_LEN, SymmetricKey};
use crate::secret::SecretString;
use crate::secret_key::AccountSecretKey;

const INFO_SECRET_KEY: &[u8] = b"quillock/v1/secret-key";
const INFO_AUK: &[u8] = b"quillock/v1/auk";

/// Everything needed to derive the AUK.
pub struct UnlockMaterial<'a> {
    /// The user's master password.
    pub password: &'a SecretString,
    /// The account Secret Key.
    pub secret_key: &'a AccountSecretKey,
    /// Account identifier (binds the Secret Key to this account).
    pub account_id: &'a [u8],
    /// Random per-account salt.
    pub salt: &'a [u8],
    /// Argon2id parameters.
    pub params: KdfParams,
}

/// Derives the Account Unlock Key. Never stored; recomputed on every full unlock.
pub fn derive_account_unlock_key(material: &UnlockMaterial<'_>) -> Result<SymmetricKey> {
    let k_pw = derive_password_key(material.password, material.salt, material.params)?;
    let k_sk = derive_key(
        material.secret_key.expose(),
        material.account_id,
        INFO_SECRET_KEY,
    )?;

    let mut ikm = Zeroizing::new([0u8; KEY_LEN * 2]);
    ikm[..KEY_LEN].copy_from_slice(k_pw.expose());
    ikm[KEY_LEN..].copy_from_slice(k_sk.expose());
    derive_key(ikm.as_ref(), material.salt, INFO_AUK)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn derive(password: &str, sk: [u8; 16], account: &[u8], salt: &[u8]) -> SymmetricKey {
        derive_account_unlock_key(&UnlockMaterial {
            password: &password.into(),
            secret_key: &AccountSecretKey::from_bytes(sk),
            account_id: account,
            salt,
            params: KdfParams::insecure_for_tests(),
        })
        .unwrap()
    }

    #[test]
    fn deterministic() {
        assert_eq!(
            derive("pw", [1; 16], b"acct", &[2; 16]),
            derive("pw", [1; 16], b"acct", &[2; 16])
        );
    }

    #[test]
    fn every_input_changes_the_key() {
        let base = derive("pw", [1; 16], b"acct", &[2; 16]);
        assert_ne!(base, derive("pw2", [1; 16], b"acct", &[2; 16]), "password");
        assert_ne!(base, derive("pw", [9; 16], b"acct", &[2; 16]), "secret key");
        assert_ne!(
            base,
            derive("pw", [1; 16], b"other", &[2; 16]),
            "account id"
        );
        assert_ne!(base, derive("pw", [1; 16], b"acct", &[3; 16]), "salt");
    }

    #[test]
    fn propagates_kdf_errors() {
        let result = derive_account_unlock_key(&UnlockMaterial {
            password: &"pw".into(),
            secret_key: &AccountSecretKey::from_bytes([1; 16]),
            account_id: b"a",
            salt: &[0; 4], // too short
            params: KdfParams::insecure_for_tests(),
        });
        assert!(result.is_err());
    }
}
