//! A Secret Key store that survives a keychain entry this app cannot change.
//!
//! macOS ties each Keychain entry to the app that made it. An entry left by a build signed
//! differently (a development build, a test build) can be read after a password prompt but cannot be
//! replaced or deleted by this app: "Invalid attempt to change the owner of this item". Erasing a
//! vault, or starting a new one, would then fail for good. When that happens the keychain entry is
//! left alone and ignored (a marker file says so) and the key lives in the owner-only key file, the
//! same fallback used where there is no keychain at all.

use std::path::PathBuf;

use password365_app::Result;
use password365_app::secret_key_store::{FileSecretKeyStore, SecretKeyStore};
use password365_crypto::secret_key::AccountSecretKey;

/// Wraps the keychain-backed store `inner` with the key file as the place to go when the keychain
/// refuses.
pub struct BypassingStore {
    inner: Box<dyn SecretKeyStore>,
    file: FileSecretKeyStore,
    marker: PathBuf,
}

impl BypassingStore {
    /// `file` is the fallback key file; `marker` records that the keychain entry is being ignored.
    #[must_use]
    pub fn new(inner: Box<dyn SecretKeyStore>, file: PathBuf, marker: PathBuf) -> Self {
        Self {
            inner,
            file: FileSecretKeyStore::new(file),
            marker,
        }
    }

    fn bypassed(&self) -> bool {
        self.marker.exists()
    }

    fn bypass(&self) {
        let _ = std::fs::write(&self.marker, b"the keychain entry is not ours to change\n");
    }
}

impl SecretKeyStore for BypassingStore {
    fn load(&self) -> Result<Option<AccountSecretKey>> {
        if self.bypassed() {
            self.file.load()
        } else {
            self.inner.load()
        }
    }

    fn save(&self, key: &AccountSecretKey) -> Result<()> {
        if self.bypassed() {
            return self.file.save(key);
        }
        if self.inner.save(key).is_ok() {
            return Ok(());
        }
        // A stale entry in the way: remove it, or, when that is refused, ignore it.
        if self.inner.remove().is_ok() && self.inner.save(key).is_ok() {
            return Ok(());
        }
        self.bypass();
        self.file.save(key)
    }

    fn remove(&self) -> Result<()> {
        if self.inner.remove().is_ok() {
            let _ = std::fs::remove_file(&self.marker);
            return Ok(());
        }
        self.bypass();
        self.file.remove()
    }
}

#[cfg(test)]
mod tests {
    use password365_app::AppError;
    use password365_app::secret_key_store::{MemorySlot, SecretSlot, SlotSecretKeyStore};

    use super::*;

    /// A slot whose entry belongs to another app: readable, never changeable.
    struct ForeignSlot(MemorySlot);

    impl SecretSlot for ForeignSlot {
        fn get(&self) -> Result<Option<String>> {
            self.0.get()
        }
        fn set(&self, _: &str) -> Result<()> {
            Err(AppError::Io(
                "Invalid attempt to change the owner of this item".into(),
            ))
        }
        fn delete(&self) -> Result<()> {
            Err(AppError::Io(
                "Invalid attempt to change the owner of this item".into(),
            ))
        }
    }

    fn store(dir: &std::path::Path, slot: impl SecretSlot + 'static) -> BypassingStore {
        BypassingStore::new(
            Box::new(SlotSecretKeyStore::new(slot)),
            dir.join("secret-key"),
            dir.join("secret-key.bypass"),
        )
    }

    #[test]
    fn a_working_keychain_is_used_and_the_key_file_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let slot = MemorySlot::default();
        let keys = store(dir.path(), slot.clone());
        let key = AccountSecretKey::generate().unwrap();
        keys.save(&key).unwrap();
        assert!(slot.get().unwrap().is_some());
        assert!(!dir.path().join("secret-key").exists());
        keys.remove().unwrap();
        assert!(keys.load().unwrap().is_none());
        assert!(!dir.path().join("secret-key.bypass").exists());
    }

    #[test]
    fn an_entry_that_is_not_ours_to_change_is_ignored_and_the_key_file_takes_over() {
        let dir = tempfile::tempdir().unwrap();
        let stale = MemorySlot::default();
        stale
            .set(&AccountSecretKey::generate().unwrap().to_display_string())
            .unwrap();
        let keys = store(dir.path(), ForeignSlot(stale.clone()));

        // Erasing cannot delete the entry: it is bypassed instead of failing.
        keys.remove().unwrap();
        assert!(dir.path().join("secret-key.bypass").exists());
        assert!(
            keys.load().unwrap().is_none(),
            "the stale key is not offered any more"
        );

        // A new vault's key goes to the file, and is what loads.
        let fresh = AccountSecretKey::generate().unwrap();
        keys.save(&fresh).unwrap();
        assert!(dir.path().join("secret-key").exists());
        assert_eq!(keys.load().unwrap(), Some(fresh));
        assert!(
            keys.save(&AccountSecretKey::generate().unwrap()).is_err(),
            "no overwrite"
        );

        // Erasing again clears the file and keeps ignoring the entry.
        keys.remove().unwrap();
        assert!(keys.load().unwrap().is_none());
    }

    #[test]
    fn saving_over_a_stale_foreign_entry_falls_back_to_the_file_without_an_erase() {
        let dir = tempfile::tempdir().unwrap();
        let stale = MemorySlot::default();
        stale
            .set(&AccountSecretKey::generate().unwrap().to_display_string())
            .unwrap();
        let keys = store(dir.path(), ForeignSlot(stale));
        let fresh = AccountSecretKey::generate().unwrap();
        keys.save(&fresh).unwrap();
        assert_eq!(keys.load().unwrap(), Some(fresh));
    }
}
