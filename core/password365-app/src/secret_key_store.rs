//! Where this device keeps its Secret Key.
//!
//! The store file and the Secret Key are deliberately kept apart so a copy of
//! the store alone cannot be attacked. Implementations: an owner-only file
//! (CLI, desktop fallback), in-memory (tests); OS keychains come later.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use password365_crypto::secret_key::AccountSecretKey;
use password365_vault::private_file::create_private_file;

use crate::{AppError, Result};

/// Persistent storage for the device's Secret Key.
pub trait SecretKeyStore: Send {
    /// Loads the key, or `None` if none is stored.
    fn load(&self) -> Result<Option<AccountSecretKey>>;
    /// Stores the key. Must refuse to overwrite an existing key.
    fn save(&self, key: &AccountSecretKey) -> Result<()>;
    /// Removes the stored key (used to roll back a failed account creation).
    fn remove(&self) -> Result<()>;
}

/// Secret Key in an owner-only (0600) text file.
pub struct FileSecretKeyStore {
    path: PathBuf,
}

impl FileSecretKeyStore {
    /// Uses the file at `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SecretKeyStore for FileSecretKeyStore {
    fn load(&self) -> Result<Option<AccountSecretKey>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&self.path)?;
        Ok(Some(AccountSecretKey::parse(&text)?))
    }

    fn save(&self, key: &AccountSecretKey) -> Result<()> {
        let mut file = create_private_file(&self.path)
            .map_err(|e| AppError::Io(format!("cannot create {}: {e}", self.path.display())))?;
        writeln!(file, "{}", key.to_display_string())?;
        file.sync_all()?;
        Ok(())
    }

    fn remove(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

/// In-memory store (tests, or "don't remember this device").
#[derive(Clone, Default)]
pub struct MemorySecretKeyStore {
    key: Arc<Mutex<Option<AccountSecretKey>>>,
}

impl SecretKeyStore for MemorySecretKeyStore {
    fn load(&self) -> Result<Option<AccountSecretKey>> {
        Ok(self.lock()?.clone())
    }

    fn save(&self, key: &AccountSecretKey) -> Result<()> {
        let mut slot = self.lock()?;
        if slot.is_some() {
            return Err(AppError::Io("a Secret Key is already stored".into()));
        }
        *slot = Some(key.clone());
        Ok(())
    }

    fn remove(&self) -> Result<()> {
        *self.lock()? = None;
        Ok(())
    }
}

impl MemorySecretKeyStore {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Option<AccountSecretKey>>> {
        self.key
            .lock()
            .map_err(|_| AppError::Internal("key store poisoned".into()))
    }
}

/// A single named secret in an OS credential store (Keychain, Credential
/// Manager, Secret Service). Implemented by frontends; kept minimal so the
/// Secret Key logic below is tested without touching the real OS store.
pub trait SecretSlot: Send {
    /// Reads the secret, `None` if absent.
    fn get(&self) -> Result<Option<String>>;
    /// Writes the secret (creating or replacing it).
    fn set(&self, value: &str) -> Result<()>;
    /// Deletes the secret; absent is not an error.
    fn delete(&self) -> Result<()>;
}

/// Secret Key stored in a [`SecretSlot`] (e.g. the OS keychain).
pub struct SlotSecretKeyStore<S: SecretSlot> {
    slot: S,
}

impl<S: SecretSlot> SlotSecretKeyStore<S> {
    /// Wraps a slot.
    pub fn new(slot: S) -> Self {
        Self { slot }
    }
}

impl<S: SecretSlot> SecretKeyStore for SlotSecretKeyStore<S> {
    fn load(&self) -> Result<Option<AccountSecretKey>> {
        self.slot
            .get()?
            .map(|text| AccountSecretKey::parse(&text).map_err(Into::into))
            .transpose()
    }

    fn save(&self, key: &AccountSecretKey) -> Result<()> {
        if self.slot.get()?.is_some() {
            return Err(AppError::Io("a Secret Key is already stored".into()));
        }
        self.slot.set(&key.to_display_string())
    }

    fn remove(&self) -> Result<()> {
        self.slot.delete()
    }
}

/// Uses `primary` (keychain) and transparently migrates a key found in the
/// `legacy` store (file) into it, deleting the legacy copy.
pub struct MigratingSecretKeyStore {
    primary: Box<dyn SecretKeyStore>,
    legacy: Box<dyn SecretKeyStore>,
}

impl MigratingSecretKeyStore {
    /// Combines a primary store with a legacy one to migrate from.
    #[must_use]
    pub fn new(primary: Box<dyn SecretKeyStore>, legacy: Box<dyn SecretKeyStore>) -> Self {
        Self { primary, legacy }
    }
}

impl SecretKeyStore for MigratingSecretKeyStore {
    fn load(&self) -> Result<Option<AccountSecretKey>> {
        if let Some(key) = self.primary.load()? {
            return Ok(Some(key));
        }
        let Some(key) = self.legacy.load()? else {
            return Ok(None);
        };
        self.primary.save(&key)?;
        self.legacy.remove()?;
        Ok(Some(key))
    }

    fn save(&self, key: &AccountSecretKey) -> Result<()> {
        if self.legacy.load()?.is_some() {
            return Err(AppError::Io("a Secret Key is already stored".into()));
        }
        self.primary.save(key)
    }

    fn remove(&self) -> Result<()> {
        self.primary.remove()?;
        self.legacy.remove()
    }
}

/// In-memory [`SecretSlot`] (tests and examples).
#[derive(Clone, Default)]
pub struct MemorySlot(Arc<Mutex<Option<String>>>);

impl SecretSlot for MemorySlot {
    fn get(&self) -> Result<Option<String>> {
        Ok(self.lock()?.clone())
    }

    fn set(&self, value: &str) -> Result<()> {
        *self.lock()? = Some(value.to_owned());
        Ok(())
    }

    fn delete(&self) -> Result<()> {
        *self.lock()? = None;
        Ok(())
    }
}

impl MemorySlot {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Option<String>>> {
        self.0
            .lock()
            .map_err(|_| AppError::Internal("slot poisoned".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise(store: &dyn SecretKeyStore) {
        assert!(store.load().unwrap().is_none());
        let key = AccountSecretKey::generate().unwrap();
        store.save(&key).unwrap();
        assert_eq!(store.load().unwrap(), Some(key));
        assert!(
            store.save(&AccountSecretKey::generate().unwrap()).is_err(),
            "no overwrite"
        );
        store.remove().unwrap();
        assert!(store.load().unwrap().is_none());
        store.remove().unwrap();
    }

    #[test]
    fn file_store_contract() {
        let dir = tempfile::tempdir().unwrap();
        exercise(&FileSecretKeyStore::new(dir.path().join("keys/secret-key")));
    }

    #[test]
    fn slot_store_contract() {
        exercise(&SlotSecretKeyStore::new(MemorySlot::default()));
    }

    #[test]
    fn slot_store_rejects_corrupt_value() {
        let slot = MemorySlot::default();
        slot.set("garbage").unwrap();
        assert_eq!(
            SlotSecretKeyStore::new(slot).load().unwrap_err().code(),
            "validation"
        );
    }

    #[test]
    fn migrating_store_contract() {
        exercise(&MigratingSecretKeyStore::new(
            Box::new(MemorySecretKeyStore::default()),
            Box::new(MemorySecretKeyStore::default()),
        ));
    }

    #[test]
    fn migrates_legacy_key_into_primary() {
        let primary = MemorySecretKeyStore::default();
        let legacy = MemorySecretKeyStore::default();
        let key = AccountSecretKey::generate().unwrap();
        legacy.save(&key).unwrap();
        let store =
            MigratingSecretKeyStore::new(Box::new(primary.clone()), Box::new(legacy.clone()));
        assert_eq!(store.load().unwrap(), Some(key.clone()));
        assert_eq!(primary.load().unwrap(), Some(key), "moved to primary");
        assert!(legacy.load().unwrap().is_none(), "legacy copy deleted");
    }

    #[test]
    fn migrating_store_refuses_save_when_legacy_has_key() {
        let legacy = MemorySecretKeyStore::default();
        legacy.save(&AccountSecretKey::generate().unwrap()).unwrap();
        let store = MigratingSecretKeyStore::new(
            Box::new(MemorySecretKeyStore::default()),
            Box::new(legacy),
        );
        assert!(store.save(&AccountSecretKey::generate().unwrap()).is_err());
    }

    #[test]
    fn memory_store_contract() {
        exercise(&MemorySecretKeyStore::default());
    }

    #[test]
    fn corrupt_file_is_a_validation_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("k");
        std::fs::write(&path, "garbage").unwrap();
        assert_eq!(
            FileSecretKeyStore::new(path).load().unwrap_err().code(),
            "validation"
        );
    }

    #[test]
    fn remove_reports_real_io_errors() {
        let dir = tempfile::tempdir().unwrap();
        // A directory cannot be removed with remove_file.
        let store = FileSecretKeyStore::new(dir.path().to_path_buf());
        assert!(store.remove().is_err());
    }

    #[test]
    fn save_into_unwritable_location_fails() {
        // A path below a regular file cannot be created on any system.
        let file = tempfile::NamedTempFile::new().unwrap();
        let store = FileSecretKeyStore::new(file.path().join("k"));
        assert_eq!(
            store
                .save(&AccountSecretKey::generate().unwrap())
                .unwrap_err()
                .code(),
            "io"
        );
    }
}
