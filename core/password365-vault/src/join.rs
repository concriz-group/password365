//! Joining an account from an already-paired device.
//!
//! Over the verified pairing channel the existing device sends a
//! [`JoinBundle`]: the account Secret Key, the vaults (names and keys) chosen
//! for the new device, its own header (only so the new device can check the
//! master password the user types), and who it is. The new device then
//! creates its own store, with its own salt and storage key, holding those vaults.

use std::path::Path;

use password365_crypto::kdf::KdfParams;
use password365_crypto::secret::SecretString;
use password365_crypto::secret_key::AccountSecretKey;
use password365_model::{Timestamp, VaultId};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::header::Header;
use crate::records::VaultRecord;
use crate::session_peers::Peer;
use crate::{Credentials, Result, Session, Store, VaultError, codec};

/// What a new device receives when it joins.
#[derive(Serialize, Deserialize)]
struct JoinBundle {
    header: Header,
    secret_key: [u8; 16],
    vaults: Vec<VaultRecord>,
    inviter: Peer,
    /// The inviter's latest password change: the new device starts with it
    /// already applied.
    password_change: Option<crate::PasswordChange>,
}

impl Drop for JoinBundle {
    fn drop(&mut self) {
        self.secret_key.zeroize();
    }
}

impl Session {
    /// Builds the bundle for a new device that may hold `vault_ids`.
    /// Send it only over a verified pairing channel: it contains the Secret Key
    /// and vault keys.
    pub fn join_bundle(
        &self,
        secret_key: &AccountSecretKey,
        vault_ids: &[VaultId],
    ) -> Result<Zeroizing<Vec<u8>>> {
        let vaults = vault_ids
            .iter()
            .map(|id| {
                let entry = self.vault_entry(*id)?;
                Ok(VaultRecord {
                    vault: entry.vault.clone(),
                    key: *entry.key.expose(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let keys = self.static_key();
        let bundle = JoinBundle {
            header: self.header.clone(),
            secret_key: *secret_key.expose(),
            vaults,
            inviter: Peer::new(
                self.device_id(),
                self.device_name(),
                keys.public(),
                Timestamp::now(),
            ),
            password_change: self.password_change().cloned(),
        };
        codec::encode(&bundle)
    }
}

impl Store {
    /// Creates this device's store from a received bundle. Fails with
    /// [`VaultError::WrongCredentials`] (writing nothing) if `password` is not
    /// the account's master password. Returns the session and the Secret Key
    /// for the app to keep in the system keychain.
    pub fn join(
        path: &Path,
        bundle: &[u8],
        password: SecretString,
        kdf: KdfParams,
    ) -> Result<(Session, AccountSecretKey)> {
        let bundle: JoinBundle = codec::decode(bundle).map_err(|_| VaultError::NotABundle)?;
        let credentials = Credentials {
            password,
            secret_key: AccountSecretKey::from_bytes(bundle.secret_key),
        };
        bundle.header.unlock(&credentials)?;
        let mut session = Self::create_empty(path, &credentials, kdf)?;
        for vault in &bundle.vaults {
            session.persist_vault(vault)?;
        }
        session.add_peer(bundle.inviter.clone())?;
        session.adopt_joined_password(bundle.password_change.clone())?;
        Ok((session, credentials.secret_key))
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview};

    use super::*;
    use crate::store::test_support::{credentials, new_session};

    fn secret() -> AccountSecretKey {
        credentials("master").secret_key
    }

    #[test]
    fn a_new_device_joins_with_the_chosen_vaults_and_syncs() {
        let (dir, mut a) = new_session();
        a.set_device_name("Laptop").unwrap();
        let personal = a.vault_by_name("Personal").unwrap().id;
        let work = a.create_vault("Work").unwrap();
        a.add_item(
            personal,
            ItemKind::Login,
            ItemOverview::new("Mail").unwrap(),
            ItemDetails::default(),
        )
        .unwrap();
        a.add_item(
            work,
            ItemKind::Login,
            ItemOverview::new("Job").unwrap(),
            ItemDetails::default(),
        )
        .unwrap();

        let bundle = a.join_bundle(&secret(), &[personal]).unwrap();
        let path = dir.path().join("b.qlvault");
        let (mut b, key) = Store::join(
            &path,
            &bundle,
            "master".into(),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        assert_eq!(key.expose(), secret().expose());
        let names: Vec<String> = b.vaults().iter().map(|v| v.name.clone()).collect();
        assert_eq!(names, vec!["Personal"], "only the chosen vault");
        assert_ne!(b.device_id(), a.device_id());
        let inviter = b.peer(a.device_id()).unwrap();
        assert_eq!(
            (inviter.name.as_str(), inviter.static_public),
            ("Laptop", a.static_key().public())
        );

        let report = b.apply_changes(&a.changes_since(None).unwrap()).unwrap();
        assert_eq!((report.changed.len(), report.skipped), (1, 1));
        let titles: Vec<&str> = b
            .items()
            .iter()
            .map(|i| i.overview.title.as_str())
            .collect();
        assert_eq!(titles, vec!["Mail"]);

        drop(b);
        let b = Store::open(&path)
            .unwrap()
            .unlock(&Credentials {
                password: "master".into(),
                secret_key: key,
            })
            .unwrap();
        assert_eq!(
            b.items().len(),
            1,
            "the new store opens with the same credentials"
        );
    }

    #[test]
    fn a_wrong_master_password_writes_nothing() {
        let (dir, a) = new_session();
        let bundle = a.join_bundle(&secret(), &[]).unwrap();
        let path = dir.path().join("b.qlvault");
        let err = Store::join(
            &path,
            &bundle,
            "nope".into(),
            KdfParams::insecure_for_tests(),
        );
        assert!(matches!(err, Err(VaultError::WrongCredentials)));
        assert!(!path.exists());
    }

    #[test]
    fn garbage_is_not_a_bundle_and_unknown_vaults_are_refused() {
        let (dir, a) = new_session();
        let err = Store::join(
            &dir.path().join("b"),
            b"junk",
            "master".into(),
            KdfParams::insecure_for_tests(),
        );
        assert!(matches!(err, Err(VaultError::NotABundle)));
        assert!(matches!(
            a.join_bundle(&secret(), &[VaultId::new()]),
            Err(VaultError::NotFound("vault"))
        ));
    }

    #[test]
    fn joining_never_overwrites_an_existing_store() {
        let (dir, a) = new_session();
        let bundle = a.join_bundle(&secret(), &[]).unwrap();
        let existing = dir.path().join("test.qlvault");
        let err = Store::join(
            &existing,
            &bundle,
            "master".into(),
            KdfParams::insecure_for_tests(),
        );
        assert!(matches!(err, Err(VaultError::AlreadyExists)));
    }

    #[test]
    fn a_device_joining_after_a_password_change_starts_with_it_applied() {
        let (dir, mut a) = new_session();
        let new = Credentials {
            password: "changed".into(),
            secret_key: secret(),
        };
        a.change_master_password(&new, KdfParams::insecure_for_tests())
            .unwrap();
        let vault = a.vault_by_name("Personal").unwrap().id;
        let bundle = a.join_bundle(&secret(), &[vault]).unwrap();
        let (b, _) = Store::join(
            &dir.path().join("b.qlvault"),
            &bundle,
            "changed".into(),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        assert!(b.password_change().is_some());
        assert!(b.pending_password_change().is_none());
    }
}
