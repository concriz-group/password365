//! Store header: identifies the store and holds the wrapped account keyset.

use password365_crypto::kdf::KdfParams;
use password365_crypto::key::SymmetricKey;
use password365_crypto::unlock::{UnlockMaterial, derive_account_unlock_key};
use password365_crypto::wrap::{unwrap_key, wrap_key};
use password365_crypto::{CryptoError, random};

use crate::context::{self, StoreId};
use crate::store::Credentials;
use crate::{Result, VaultError};

/// Current on-disk format version.
pub(crate) const FORMAT_VERSION: u32 = 1;
/// Salt length in bytes.
pub(crate) const SALT_LEN: usize = 32;

/// Whether a store must already hold its integrity sum (see `integrity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sums {
    /// Written by a version that keeps the sum: a store without it, or with
    /// one that is not sealed, was tampered with.
    Required,
    /// Written before the sum existed (or brand new): it gets one now.
    Adopt,
}

/// Plaintext header row (the keyset inside is encrypted).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Header {
    pub format_version: u32,
    pub store_id: StoreId,
    pub kdf: KdfParams,
    pub salt: [u8; SALT_LEN],
    /// Storage key wrapped under the Account Unlock Key.
    pub wrapped_keyset: Vec<u8>,
}

impl Header {
    /// Creates a header for a brand-new store and returns its fresh storage key.
    pub(crate) fn create(
        credentials: &Credentials,
        kdf: KdfParams,
    ) -> Result<(Self, SymmetricKey)> {
        let store_id: StoreId = random::bytes()?;
        let storage_key = SymmetricKey::generate()?;
        let header = Self::wrapping(store_id, &storage_key, credentials, kdf)?;
        Ok((header, storage_key))
    }

    /// Recovers the storage key with the given credentials.
    pub(crate) fn unlock(&self, credentials: &Credentials) -> Result<SymmetricKey> {
        self.unlock_with_sums(credentials).map(|(key, _)| key)
    }

    /// [`Self::unlock`], also telling whether this store promises a sealed
    /// integrity sum ([`Sums::Required`]) or was written before that
    /// ([`Sums::Adopt`]). The promise is in the keyset's associated data, so
    /// only someone with the master password and Secret Key can make or
    /// remove it.
    pub(crate) fn unlock_with_sums(
        &self,
        credentials: &Credentials,
    ) -> Result<(SymmetricKey, Sums)> {
        if self.format_version != FORMAT_VERSION {
            return Err(VaultError::UnsupportedFormat(self.format_version));
        }
        let auk = derive_auk(&self.store_id, &self.salt, self.kdf, credentials)?;
        let current = context::keyset_with_sum(&self.store_id, &self.salt, self.kdf);
        match unwrap_key(&auk, &self.wrapped_keyset, &current) {
            Ok(key) => return Ok((key, Sums::Required)),
            Err(CryptoError::Decrypt) => {}
            Err(other) => return Err(VaultError::Crypto(other)),
        }
        let original = context::keyset(&self.store_id, &self.salt, self.kdf);
        unwrap_key(&auk, &self.wrapped_keyset, &original)
            .map(|key| (key, Sums::Adopt))
            .map_err(|e| match e {
                CryptoError::Decrypt => VaultError::WrongCredentials,
                other => VaultError::Crypto(other),
            })
    }

    /// Re-wraps the storage key under new credentials (fresh salt).
    pub(crate) fn rewrap(
        &self,
        storage_key: &SymmetricKey,
        credentials: &Credentials,
        kdf: KdfParams,
    ) -> Result<Self> {
        Self::wrapping(self.store_id, storage_key, credentials, kdf)
    }

    fn wrapping(
        store_id: StoreId,
        storage_key: &SymmetricKey,
        credentials: &Credentials,
        kdf: KdfParams,
    ) -> Result<Self> {
        let salt: [u8; SALT_LEN] = random::bytes()?;
        let auk = derive_auk(&store_id, &salt, kdf, credentials)?;
        let wrapped_keyset = wrap_key(
            &auk,
            storage_key,
            &context::keyset_with_sum(&store_id, &salt, kdf),
        )?;
        Ok(Self {
            format_version: FORMAT_VERSION,
            store_id,
            kdf,
            salt,
            wrapped_keyset,
        })
    }
}

#[cfg(test)]
impl Header {
    /// This header as written before the integrity sum existed (no promise).
    pub(crate) fn before_sums(&self, credentials: &Credentials) -> Self {
        let key = self.unlock(credentials).unwrap();
        let auk = derive_auk(&self.store_id, &self.salt, self.kdf, credentials).unwrap();
        let aad = context::keyset(&self.store_id, &self.salt, self.kdf);
        Self {
            wrapped_keyset: wrap_key(&auk, &key, &aad).unwrap(),
            ..self.clone()
        }
    }
}

fn derive_auk(
    store_id: &StoreId,
    salt: &[u8],
    params: KdfParams,
    credentials: &Credentials,
) -> Result<SymmetricKey> {
    Ok(derive_account_unlock_key(&UnlockMaterial {
        password: &credentials.password,
        secret_key: &credentials.secret_key,
        account_id: store_id,
        salt,
        params,
    })?)
}

#[cfg(test)]
mod tests {
    use password365_crypto::secret_key::AccountSecretKey;

    use super::*;

    fn creds(pw: &str) -> Credentials {
        Credentials {
            password: pw.into(),
            secret_key: AccountSecretKey::from_bytes([4; 16]),
        }
    }

    fn fast() -> KdfParams {
        KdfParams::insecure_for_tests()
    }

    #[test]
    fn create_then_unlock_returns_same_key() {
        let (header, key) = Header::create(&creds("pw"), fast()).unwrap();
        assert_eq!(header.unlock(&creds("pw")).unwrap(), key);
    }

    #[test]
    fn a_header_says_whether_the_store_promises_its_integrity_sum() {
        let (header, key) = Header::create(&creds("pw"), fast()).unwrap();
        assert_eq!(
            header.unlock_with_sums(&creds("pw")).unwrap(),
            (key.clone(), Sums::Required)
        );
        // One written before the sum existed still opens, without the promise.
        let old = header.before_sums(&creds("pw"));
        assert_eq!(
            old.unlock_with_sums(&creds("pw")).unwrap(),
            (key, Sums::Adopt)
        );
        assert!(matches!(
            old.unlock_with_sums(&creds("nope")),
            Err(VaultError::WrongCredentials)
        ));
    }

    #[test]
    fn wrong_password_is_wrong_credentials() {
        let (header, _) = Header::create(&creds("pw"), fast()).unwrap();
        assert!(matches!(
            header.unlock(&creds("nope")),
            Err(VaultError::WrongCredentials)
        ));
    }

    #[test]
    fn wrong_secret_key_is_wrong_credentials() {
        let (header, _) = Header::create(&creds("pw"), fast()).unwrap();
        let other = Credentials {
            password: "pw".into(),
            secret_key: AccountSecretKey::from_bytes([5; 16]),
        };
        assert!(matches!(
            header.unlock(&other),
            Err(VaultError::WrongCredentials)
        ));
    }

    #[test]
    fn tampered_salt_fails() {
        let (mut header, _) = Header::create(&creds("pw"), fast()).unwrap();
        header.salt[0] ^= 1;
        assert!(matches!(
            header.unlock(&creds("pw")),
            Err(VaultError::WrongCredentials)
        ));
    }

    #[test]
    fn downgraded_kdf_params_are_refused() {
        let (mut header, _) = Header::create(&creds("pw"), fast()).unwrap();
        header.kdf = KdfParams {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        assert!(matches!(
            header.unlock(&creds("pw")),
            Err(VaultError::Crypto(CryptoError::WeakKdfParams(_)))
        ));
    }

    #[test]
    fn unknown_format_is_refused() {
        let (mut header, _) = Header::create(&creds("pw"), fast()).unwrap();
        header.format_version = 99;
        assert!(matches!(
            header.unlock(&creds("pw")),
            Err(VaultError::UnsupportedFormat(99))
        ));
    }

    #[test]
    fn rewrap_changes_salt_and_credentials() {
        let (header, key) = Header::create(&creds("old"), fast()).unwrap();
        let new = header.rewrap(&key, &creds("new"), fast()).unwrap();
        assert_eq!(new.store_id, header.store_id);
        assert_ne!(new.salt, header.salt);
        assert_eq!(new.unlock(&creds("new")).unwrap(), key);
        assert!(new.unlock(&creds("old")).is_err());
    }
}
