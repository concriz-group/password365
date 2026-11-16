//! Quick unlock (Face ID / Touch ID): after a master-password unlock, the
//! store's key is wrapped by a random key that only this device keeps — on
//! iOS in the Keychain, readable only after biometry. The wrapped copy is
//! bound to this store and to the current master password's salt (a
//! password change invalidates it) and to when it was made (it expires).
//!
//! Layout: `version (1) || made_at ms (8, big-endian) || wrapped key`.

use password365_crypto::aad::Aad;
use password365_crypto::key::SymmetricKey;
use password365_crypto::wrap::{unwrap_key, wrap_key};
use password365_model::Timestamp;

use crate::store::Store;
use crate::{Result, Session, VaultError};

const VERSION: u8 = 1;

fn context(store_id: &[u8; 16], salt: &[u8], made_at: i64) -> Aad {
    Aad::new("password365/v1/quick-unlock")
        .bytes(store_id)
        .bytes(salt)
        .u64(u64::try_from(made_at).unwrap_or_default())
}

impl Session {
    /// Wraps this store's key for a later quick unlock with `device_key`.
    pub fn seal_quick_unlock(&self, device_key: &SymmetricKey, now: Timestamp) -> Result<Vec<u8>> {
        let made_at = now.as_millis();
        let aad = context(&self.header.store_id, &self.header.salt, made_at);
        let mut out = vec![VERSION];
        out.extend_from_slice(&made_at.to_be_bytes());
        out.extend(wrap_key(device_key, &self.storage_key, &aad)?);
        Ok(out)
    }
}

impl Store {
    /// Unlocks with a quick-unlock copy made less than `max_age_ms` ago.
    /// `QuickUnlockExpired` asks for the master password instead.
    pub fn unlock_quick(
        self,
        device_key: &SymmetricKey,
        sealed: &[u8],
        now: Timestamp,
        max_age_ms: i64,
    ) -> Result<Session> {
        let (&version, rest) = sealed
            .split_first()
            .ok_or(VaultError::Corrupt("quick unlock"))?;
        if version != VERSION || rest.len() < 8 {
            return Err(VaultError::Corrupt("quick unlock"));
        }
        let (made, wrapped) = rest.split_at(8);
        let made_at = i64::from_be_bytes(
            made.try_into()
                .map_err(|_| VaultError::Corrupt("quick unlock"))?,
        );
        let age = now.as_millis() - made_at;
        if !(0..=max_age_ms).contains(&age) {
            return Err(VaultError::QuickUnlockExpired);
        }
        let aad = context(&self.header.store_id, &self.header.salt, made_at);
        let storage_key =
            unwrap_key(device_key, wrapped, &aad).map_err(|_| VaultError::QuickUnlockExpired)?;
        Session::load(self.db, self.header, storage_key)
    }
}

#[cfg(test)]
mod tests {
    use password365_crypto::kdf::KdfParams;

    use super::*;
    use crate::Credentials;
    use crate::store::test_support::credentials;

    const DAY: i64 = 24 * 60 * 60 * 1000;

    #[test]
    fn opens_with_the_device_key_until_it_expires_or_the_password_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s");
        let mut session = Store::create(
            &path,
            &credentials("master"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        session.create_vault("Travel").unwrap();
        let device_key = SymmetricKey::generate().unwrap();
        let made = Timestamp::from_millis(1_000 * DAY);
        let sealed = session.seal_quick_unlock(&device_key, made).unwrap();
        drop(session);

        let opened = Store::open(&path)
            .unwrap()
            .unlock_quick(
                &device_key,
                &sealed,
                Timestamp::from_millis(1_013 * DAY),
                14 * DAY,
            )
            .unwrap();
        assert!(
            opened.vault_by_name("Travel").is_some(),
            "the real store, decrypted"
        );
        drop(opened);

        let late = Store::open(&path).unwrap().unlock_quick(
            &device_key,
            &sealed,
            Timestamp::from_millis(1_015 * DAY),
            14 * DAY,
        );
        assert!(matches!(late, Err(VaultError::QuickUnlockExpired)));
        let other = SymmetricKey::generate().unwrap();
        let wrong = Store::open(&path)
            .unwrap()
            .unlock_quick(&other, &sealed, made, 14 * DAY);
        assert!(matches!(wrong, Err(VaultError::QuickUnlockExpired)));
        let mut tampered = sealed.clone();
        tampered[3] ^= 1; // another "made at": the binding fails
        assert!(
            Store::open(&path)
                .unwrap()
                .unlock_quick(&device_key, &tampered, made, 14 * DAY)
                .is_err()
        );
        assert!(
            Store::open(&path)
                .unwrap()
                .unlock_quick(&device_key, &[1, 2], made, 14 * DAY)
                .is_err()
        );

        // A new master password: the old quick-unlock copy no longer opens.
        let mut session = Store::open(&path)
            .unwrap()
            .unlock(&credentials("master"))
            .unwrap();
        let new = Credentials {
            password: "changed".into(),
            secret_key: credentials("x").secret_key,
        };
        session
            .change_master_password(&new, KdfParams::insecure_for_tests())
            .unwrap();
        drop(session);
        let after = Store::open(&path)
            .unwrap()
            .unlock_quick(&device_key, &sealed, made, 14 * DAY);
        assert!(matches!(after, Err(VaultError::QuickUnlockExpired)));
    }
}
