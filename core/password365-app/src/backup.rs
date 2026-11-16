//! Encrypted backups: export the store to a file, restore it on a new device,
//! and remind the user when the last backup is too old.
//!
//! A backup is a consistent copy of the store file — exactly as encrypted as
//! the store. Restoring needs the file + the Secret Key (Emergency Kit) + the
//! master password.

use std::path::{Path, PathBuf};

use password365_crypto::secret::SecretString;
use password365_crypto::secret_key::AccountSecretKey;
use password365_model::Timestamp;
use password365_vault::{Credentials, Store};
use serde::{Deserialize, Serialize};

use crate::service::AppService;
use crate::{AppError, Result};

/// Remind the user after this long without a backup.
pub const BACKUP_DUE_AFTER_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Suggested file extension.
pub const BACKUP_EXTENSION: &str = "p365backup";
const LOG_FILE: &str = "backup-log.json";

/// When the last backup was made (not secret, not a user setting).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupLog {
    pub(crate) last_backup_ms: Option<i64>,
}

/// What the UI shows about backups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BackupInfo {
    /// Last export (ms since epoch), if any.
    pub last_backup_ms: Option<i64>,
    /// `true` when there is no backup or it is older than 30 days.
    pub due: bool,
}

impl BackupLog {
    fn info(self, now: Timestamp) -> BackupInfo {
        BackupInfo {
            last_backup_ms: self.last_backup_ms,
            due: self
                .last_backup_ms
                .is_none_or(|t| now.as_millis() - t > BACKUP_DUE_AFTER_MS),
        }
    }
}

/// The log lives next to the settings file (in memory only without one).
pub(crate) fn log_path(settings_path: Option<&Path>) -> Option<PathBuf> {
    settings_path.map(|p| p.with_file_name(LOG_FILE))
}

pub(crate) fn load_log(settings_path: Option<&Path>) -> BackupLog {
    log_path(settings_path)
        .and_then(|p| crate::json_file::read(&p))
        .unwrap_or_default()
}

impl AppService {
    /// Writes an encrypted backup of everything to `path` (vault must be unlocked).
    pub fn export_backup(&mut self, path: &Path) -> Result<BackupInfo> {
        self.session()?.backup_to(path)?;
        let now = Timestamp::now();
        self.backup_log.last_backup_ms = Some(now.as_millis());
        if let Some(log) = log_path(self.config.settings_path.as_deref()) {
            crate::json_file::write(&log, &self.backup_log)?;
        }
        Ok(self.backup_log.info(now))
    }

    /// Last backup and whether a new one is due.
    #[must_use]
    pub fn backup_info(&self) -> BackupInfo {
        self.backup_log.info(Timestamp::now())
    }

    /// First run on a new device: restores `backup` with the Secret Key and
    /// master password, remembers the Secret Key here, and leaves it unlocked.
    pub fn restore_backup(
        &mut self,
        backup: &Path,
        password: SecretString,
        secret_key: &str,
    ) -> Result<()> {
        if self.status().initialized {
            return Err(AppError::AlreadyInitialized);
        }
        let secret_key = AccountSecretKey::parse(secret_key)?;
        let credentials = Credentials {
            password,
            secret_key,
        };
        let session = Store::restore_from(backup, &self.config.store_path, &credentials)?;
        self.keys.save(&credentials.secret_key)?;
        self.start_session(session);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use password365_crypto::kdf::KdfParams;

    use super::*;
    use crate::AppConfig;
    use crate::item_input::ItemInput;
    use crate::secret_key_store::MemorySecretKeyStore;

    const PW: &str = "long enough password";

    fn service(dir: &Path, with_settings: bool) -> AppService {
        let config = AppConfig {
            store_path: dir.join("store.qlvault"),
            kdf: KdfParams::insecure_for_tests(),
            auto_lock_ms: 0,
            settings_path: with_settings.then(|| dir.join("settings.json")),
        };
        AppService::new(config, Box::new(MemorySecretKeyStore::default()))
    }

    #[test]
    fn backup_then_restore_on_a_new_device() {
        let old = tempfile::tempdir().unwrap();
        let mut app = service(old.path(), true);
        assert!(app.backup_info().due, "never backed up");
        let key = app.create_account(PW.into(), &PW.into()).unwrap();
        app.create_item(&ItemInput {
            title: "Bank".into(),
            password: Some("1234".into()),
            ..ItemInput::default()
        })
        .unwrap();
        let file = old.path().join("backup.p365backup");
        let info = app.export_backup(&file).unwrap();
        assert!(!info.due);
        assert!(info.last_backup_ms.is_some());
        // Remembered across restarts.
        assert_eq!(service(old.path(), true).backup_info(), info);

        let new = tempfile::tempdir().unwrap();
        let mut fresh = service(new.path(), false);
        let wrong = fresh.restore_backup(&file, "wrong password!!".into(), &key);
        assert_eq!(wrong.unwrap_err(), AppError::WrongCredentials);
        assert!(!fresh.status().initialized);

        fresh.restore_backup(&file, PW.into(), &key).unwrap();
        assert!(fresh.status().unlocked);
        let items = fresh.items("bank", None).unwrap();
        assert_eq!(items.len(), 1);
        // The Secret Key is now remembered on this device.
        fresh.lock();
        fresh.unlock(PW.into(), None).unwrap();
    }

    #[test]
    fn restore_refuses_existing_accounts_and_bad_input() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = service(dir.path(), false);
        let key = app.create_account(PW.into(), &PW.into()).unwrap();
        let file = dir.path().join("b.p365backup");
        app.export_backup(&file).unwrap();
        assert_eq!(
            app.restore_backup(&file, PW.into(), &key).unwrap_err(),
            AppError::AlreadyInitialized
        );

        let other = tempfile::tempdir().unwrap();
        let mut fresh = service(other.path(), false);
        assert_eq!(
            fresh
                .restore_backup(&file, PW.into(), "not a key")
                .unwrap_err()
                .code(),
            "validation"
        );
        let junk = other.path().join("junk.p365backup");
        std::fs::write(&junk, b"junk").unwrap();
        let err = fresh.restore_backup(&junk, PW.into(), &key).unwrap_err();
        assert_eq!(err.code(), "validation");
        assert!(err.to_string().contains("not a Password365 backup"));
    }

    #[test]
    fn export_needs_an_unlocked_vault() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = service(dir.path(), false);
        app.create_account(PW.into(), &PW.into()).unwrap();
        app.lock();
        let err = app.export_backup(&dir.path().join("x")).unwrap_err();
        assert_eq!(err, AppError::Locked);
    }

    #[test]
    fn backups_become_due_after_30_days() {
        let log = BackupLog {
            last_backup_ms: Some(0),
        };
        assert!(!log.info(Timestamp::from_millis(BACKUP_DUE_AFTER_MS)).due);
        assert!(
            log.info(Timestamp::from_millis(BACKUP_DUE_AFTER_MS + 1))
                .due
        );
        assert_eq!(log_path(None), None);
    }
}
