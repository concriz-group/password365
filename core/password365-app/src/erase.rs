//! Erasing this device's vault: the way out when the master password is forgotten, or when the
//! vault on this device should give way to another one (join a paired device, restore a backup).

use std::path::{Path, PathBuf};

use crate::attempts::Attempts;
use crate::{AppError, AppService, Result};

/// Files that belong to the store, beside it: SQLite's side files and the quick-unlock copy.
const SIDE_FILES: [&str; 5] = ["", "-wal", "-shm", "-journal", ".quick"];

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(suffix);
    PathBuf::from(os)
}

impl AppService {
    /// Erases the vault on this device for good: the encrypted store, the quick-unlock copy, the
    /// Secret Key in the keychain and the count of wrong passwords. Works while locked (that is the
    /// point: nothing here needs the password). Other paired devices keep their own copies. The
    /// caller asks the person to confirm first, and clears whatever its platform added (Face ID
    /// key, preferences).
    pub fn erase_vault(&mut self) -> Result<()> {
        self.session = None;
        for suffix in SIDE_FILES {
            let path = with_suffix(&self.config.store_path, suffix);
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(AppError::Io(format!(
                        "cannot erase {}: {e}",
                        path.display()
                    )));
                }
            }
        }
        self.keys.remove()?;
        // What the app kept beside the settings about this vault (not preferences): when the
        // last backup was made, which identity and card the extension fills by default.
        if let Some(settings) = self.config.settings_path.as_deref() {
            for name in ["backup-log.json", "profile.json"] {
                let _ = std::fs::remove_file(settings.with_file_name(name));
            }
        }
        self.backup_log = crate::backup::BackupLog::default();
        self.profile_choice = crate::profile::ProfileChoice::default();
        self.attempts = Attempts::default();
        crate::attempts::save(self.config.settings_path.as_deref(), &self.attempts);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::secret_key_store::SecretKeyStore as _;
    use crate::service::tests::{PW, login, with_account};

    #[test]
    fn erasing_leaves_a_device_that_can_start_over() {
        let mut fx = with_account();
        fx.app.create_item(&login("A", "pw")).unwrap();
        fx.app.enable_quick_unlock().unwrap();
        assert!(fx.app.status().initialized && fx.app.quick_unlock_ready());

        fx.app.erase_vault().unwrap();
        let status = fx.app.status();
        assert!(!status.initialized && !status.unlocked);
        assert!(!fx.app.quick_unlock_ready());
        assert!(
            fx.keys.load().unwrap().is_none(),
            "the Secret Key is gone too"
        );

        // A new account can be created right away.
        fx.app.create_account(PW.into(), &PW.into()).unwrap();
        assert!(fx.app.items("", None).unwrap().is_empty());
    }

    #[test]
    fn erasing_works_while_locked_and_when_nothing_is_there() {
        let mut fx = with_account();
        fx.app.lock();
        fx.app.erase_vault().unwrap();
        assert!(!fx.app.status().initialized);
        fx.app.erase_vault().unwrap();
    }
}
