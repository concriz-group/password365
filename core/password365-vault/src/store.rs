//! Entry point: create a new store or open and unlock an existing one.

use std::path::Path;

use password365_crypto::kdf::KdfParams;
use password365_crypto::secret::SecretString;
use password365_crypto::secret_key::AccountSecretKey;

use crate::db::Database;
use crate::header::Header;
use crate::session::Session;
use crate::{Result, VaultError};

/// Name of the vault created together with a new store.
pub const DEFAULT_VAULT_NAME: &str = "Personal";

/// The two secrets needed to unlock a store.
pub struct Credentials {
    /// Master password.
    pub password: SecretString,
    /// Account Secret Key.
    pub secret_key: AccountSecretKey,
}

/// A locked store on disk.
pub struct Store {
    pub(crate) db: Database,
    pub(crate) header: Header,
}

impl Store {
    /// Creates a new store at `path` with a default "Personal" vault and returns
    /// it unlocked. Fails if a store already exists there.
    pub fn create(path: &Path, credentials: &Credentials, kdf: KdfParams) -> Result<Session> {
        let mut session = Self::create_empty(path, credentials, kdf)?;
        session.create_vault(DEFAULT_VAULT_NAME)?;
        Ok(session)
    }

    /// A new store with no vaults yet.
    pub(crate) fn create_empty(
        path: &Path,
        credentials: &Credentials,
        kdf: KdfParams,
    ) -> Result<Session> {
        let (header, storage_key) = Header::create(credentials, kdf)?;
        let db = Database::create(path)?;
        db.write_header(&header)?;
        Session::load(db, header, storage_key)
    }

    /// Opens an existing (locked) store.
    pub fn open(path: &Path) -> Result<Self> {
        let db = Database::open(path)?;
        let header = db.read_header()?;
        Ok(Self { db, header })
    }

    /// KDF parameters of this store (readable before unlocking).
    #[must_use]
    pub fn kdf_params(&self) -> KdfParams {
        self.header.kdf
    }

    /// Unlocks the store.
    pub fn unlock(self, credentials: &Credentials) -> Result<Session> {
        let storage_key = self.header.unlock(credentials)?;
        Session::load(self.db, self.header, storage_key)
    }

    /// Restores a backup to `dest` and returns it unlocked. The credentials are
    /// checked against the backup *before* anything is written, so a wrong file
    /// or wrong password leaves nothing behind. Fails if `dest` already exists.
    pub fn restore_from(backup: &Path, dest: &Path, credentials: &Credentials) -> Result<Session> {
        if dest.exists() {
            return Err(VaultError::AlreadyExists);
        }
        let source = Self::open(backup).map_err(|e| match e {
            VaultError::Database(_) | VaultError::Corrupt(_) => VaultError::NotABackup,
            other => other,
        })?;
        let storage_key = source.header.unlock(credentials)?;
        source.db.backup_to(dest)?;
        drop(source);
        let db = Database::open(dest)?;
        let header = db.read_header()?;
        let mut session = Session::load(db, header, storage_key)?;
        session.reset_device_identity()?;
        Ok(session)
    }
}

/// Helpers shared by unit tests in this crate.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) fn credentials(password: &str) -> Credentials {
        Credentials {
            password: password.into(),
            secret_key: AccountSecretKey::from_bytes([1; 16]),
        }
    }

    /// A fresh store in a temp dir (keep the `TempDir` alive while using the session).
    pub(crate) fn new_session() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let session = Store::create(
            &dir.path().join("test.qlvault"),
            &credentials("master"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        (dir, session)
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview};

    use super::test_support::credentials;
    use super::*;

    fn path(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("s.qlvault")
    }

    #[test]
    fn data_survives_lock_and_unlock() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::create(
            &path(&dir),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let v = s.vault_by_name("Personal").unwrap().id;
        let id = s
            .add_item(
                v,
                ItemKind::Login,
                ItemOverview::new("Bank").unwrap(),
                ItemDetails::with_password("123".into()),
            )
            .unwrap();
        s.lock();

        let store = Store::open(&path(&dir)).unwrap();
        assert_eq!(store.kdf_params(), KdfParams::insecure_for_tests());
        let s = store.unlock(&credentials("pw")).unwrap();
        assert_eq!(
            s.item(id).unwrap().details.password.unwrap().expose(),
            "123"
        );
    }

    #[test]
    fn wrong_password_cannot_unlock() {
        let dir = tempfile::tempdir().unwrap();
        Store::create(
            &path(&dir),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let result = Store::open(&path(&dir))
            .unwrap()
            .unlock(&credentials("bad"));
        assert!(matches!(result, Err(VaultError::WrongCredentials)));
    }

    #[test]
    fn create_twice_fails() {
        let dir = tempfile::tempdir().unwrap();
        Store::create(
            &path(&dir),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let again = Store::create(
            &path(&dir),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        );
        assert!(matches!(again, Err(VaultError::AlreadyExists)));
    }

    #[test]
    fn open_missing_store_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Store::open(&path(&dir)),
            Err(VaultError::NotInitialized)
        ));
    }

    #[test]
    fn change_credentials_takes_effect() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::create(
            &path(&dir),
            &credentials("old"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        s.change_credentials(&credentials("new"), KdfParams::insecure_for_tests())
            .unwrap();
        assert_eq!(s.kdf_params(), KdfParams::insecure_for_tests());
        s.lock();
        assert!(
            Store::open(&path(&dir))
                .unwrap()
                .unlock(&credentials("old"))
                .is_err()
        );
        let s = Store::open(&path(&dir))
            .unwrap()
            .unlock(&credentials("new"))
            .unwrap();
        assert_eq!(s.vaults().len(), 1);
    }

    fn raw(dir: &tempfile::TempDir) -> rusqlite::Connection {
        rusqlite::Connection::open(path(dir)).unwrap()
    }

    #[test]
    fn records_put_back_or_removed_behind_the_apps_back_are_noticed() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::create(
            &path(&dir),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let v = s.vault_by_name("Personal").unwrap().id;
        let id = s
            .add_item(
                v,
                password365_model::ItemKind::Login,
                password365_model::ItemOverview::new("Bank").unwrap(),
                password365_model::ItemDetails::with_password("old".into()),
            )
            .unwrap();
        let before: Vec<(Vec<u8>, Vec<u8>)> = {
            let conn = raw(&dir);
            let mut st = conn.prepare("SELECT id, blob FROM records").unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        let mut item = s.item(id).unwrap();
        item.details.password = Some("new".into());
        item.touch(password365_model::Timestamp::now());
        s.save_item(&item).unwrap();
        s.lock();
        let unlock = || Store::open(&path(&dir)).unwrap().unlock(&credentials("pw"));
        assert!(unlock().is_ok(), "untouched: opens");

        // An older copy of the records put back: noticed.
        let conn = raw(&dir);
        for (rid, blob) in &before {
            conn.execute(
                "INSERT OR REPLACE INTO records (id, blob) VALUES (?1, ?2)",
                rusqlite::params![rid, blob],
            )
            .unwrap();
        }
        drop(conn);
        assert!(matches!(unlock(), Err(VaultError::Tampered)));

        // A record removed: noticed too.
        let dir2 = tempfile::tempdir().unwrap();
        let s = Store::create(
            &path(&dir2),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        s.lock();
        raw(&dir2)
            .execute(
                "DELETE FROM records WHERE rowid IN (SELECT rowid FROM records LIMIT 1)",
                [],
            )
            .ok();
        raw(&dir2)
            .execute(
                "DELETE FROM records WHERE id = (SELECT id FROM records LIMIT 1)",
                [],
            )
            .unwrap();
        assert!(matches!(
            Store::open(&path(&dir2))
                .unwrap()
                .unlock(&credentials("pw")),
            Err(VaultError::Tampered)
        ));
    }

    #[test]
    fn a_store_made_before_the_sum_existed_gets_one_on_unlock() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::create(
            &path(&dir),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        s.lock();
        raw(&dir).execute("DROP TABLE integrity", []).unwrap();
        let s = Store::open(&path(&dir))
            .unwrap()
            .unlock(&credentials("pw"))
            .unwrap();
        s.lock();
        let rows: i64 = raw(&dir)
            .query_row("SELECT count(*) FROM integrity", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
        assert!(
            Store::open(&path(&dir))
                .unwrap()
                .unlock(&credentials("pw"))
                .is_ok()
        );
    }

    #[test]
    fn an_old_header_and_the_old_password_do_not_open_the_store_after_a_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::create(
            &path(&dir),
            &credentials("old"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let v = s.vault_by_name("Personal").unwrap().id;
        s.add_item(
            v,
            password365_model::ItemKind::Login,
            password365_model::ItemOverview::new("Bank").unwrap(),
            password365_model::ItemDetails::with_password("p".into()),
        )
        .unwrap();
        let old_header = Store::open(&path(&dir)).unwrap().header;
        let old_key = old_header.unlock(&credentials("old")).unwrap();
        s.change_credentials(&credentials("new"), KdfParams::insecure_for_tests())
            .unwrap();
        // The attacker's old header gives the old key: no current record opens with it.
        let db = Database::open(&path(&dir)).unwrap();
        let records = db.records().unwrap();
        assert!(!records.is_empty());
        for (id, blob) in records {
            assert!(crate::records::open(&old_key, &old_header.store_id, &id, &blob).is_err());
        }
        s.lock();
        let s = Store::open(&path(&dir))
            .unwrap()
            .unlock(&credentials("new"))
            .unwrap();
        assert_eq!(
            s.items().len(),
            1,
            "everything still opens with the new password"
        );
    }

    #[test]
    fn file_contains_no_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::create(
            &path(&dir),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let v = s.vault_by_name("Personal").unwrap().id;
        s.add_item(
            v,
            ItemKind::Login,
            ItemOverview::new("VerySecretBankTitle")
                .unwrap()
                .with_username("alice@example.com"),
            ItemDetails::with_password("Tr0ub4dor&3-PLAINTEXT".into()),
        )
        .unwrap();
        s.lock();
        let bytes = std::fs::read(path(&dir)).unwrap();
        for needle in [
            "VerySecretBankTitle",
            "alice@example.com",
            "Tr0ub4dor",
            "Personal",
        ] {
            assert!(
                !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "{needle} leaked"
            );
        }
    }

    #[test]
    fn foreign_records_are_ignored_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::create(
            &path(&dir),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        s.db.put_record(&[0xEE; 16], &[0xAB; 300]).unwrap();
        s.lock();
        let s = Store::open(&path(&dir))
            .unwrap()
            .unlock(&credentials("pw"))
            .unwrap();
        assert_eq!(s.vaults().len(), 1);
        assert_eq!(
            s.db.records().unwrap().len(),
            4,
            "vault + device + device keys + the foreign record, which must not be deleted"
        );
    }

    fn store_with_item(dir: &tempfile::TempDir) -> (Session, password365_model::ItemId) {
        let mut s = Store::create(
            &path(dir),
            &credentials("pw"),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let v = s.vault_by_name("Personal").unwrap().id;
        let id = s
            .add_item(
                v,
                ItemKind::Login,
                ItemOverview::new("Bank").unwrap(),
                ItemDetails::with_password("123".into()),
            )
            .unwrap();
        (s, id)
    }

    #[test]
    fn backup_restores_on_a_new_device() {
        let dir = tempfile::tempdir().unwrap();
        let (session, id) = store_with_item(&dir);
        let backup = dir.path().join("out/b.p365backup");
        session.backup_to(&backup).unwrap();
        session.backup_to(&backup).unwrap(); // replacing an older backup is fine
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&backup).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(!dir.path().join("out/b.p365backup.tmp").exists());

        let new_device = tempfile::tempdir().unwrap();
        let dest = new_device.path().join("data/restored.qlvault");
        let restored = Store::restore_from(&backup, &dest, &credentials("pw")).unwrap();
        assert_eq!(
            restored
                .item(id)
                .unwrap()
                .details
                .password
                .unwrap()
                .expose(),
            "123"
        );
        assert!(dest.is_file());
    }

    #[test]
    fn restore_checks_credentials_before_writing() {
        let dir = tempfile::tempdir().unwrap();
        let (session, _) = store_with_item(&dir);
        let backup = dir.path().join("b.p365backup");
        session.backup_to(&backup).unwrap();
        let dest = dir.path().join("restored.qlvault");

        let wrong = Store::restore_from(&backup, &dest, &credentials("nope"));
        assert!(matches!(wrong, Err(VaultError::WrongCredentials)));
        assert!(!dest.exists());

        let taken = Store::restore_from(&backup, &path(&dir), &credentials("pw"));
        assert!(matches!(taken, Err(VaultError::AlreadyExists)));
    }

    #[test]
    fn restore_rejects_files_that_are_not_backups() {
        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("photo.jpg");
        std::fs::write(&junk, b"definitely not sqlite, just some bytes").unwrap();
        let dest = dir.path().join("r.qlvault");
        let r = Store::restore_from(&junk, &dest, &credentials("pw"));
        assert!(matches!(r, Err(VaultError::NotABackup)), "{:?}", r.err());

        let empty_db = dir.path().join("empty.sqlite");
        rusqlite::Connection::open(&empty_db)
            .unwrap()
            .execute_batch("CREATE TABLE t (x);")
            .unwrap();
        let r = Store::restore_from(&empty_db, &dest, &credentials("pw"));
        assert!(matches!(r, Err(VaultError::NotABackup)), "{:?}", r.err());

        let missing = Store::restore_from(&dir.path().join("none"), &dest, &credentials("pw"));
        assert!(matches!(missing, Err(VaultError::NotInitialized)));
        assert!(!dest.exists());
    }
}
