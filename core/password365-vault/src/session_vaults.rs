//! Vault operations on an unlocked [`Session`].

use password365_crypto::key::SymmetricKey;
use password365_model::{Timestamp, Vault, VaultId};

use crate::records::{RecordBody, VaultRecord};
use crate::session::Session;
use crate::{Result, VaultError};

impl Session {
    /// All vaults, sorted by name.
    #[must_use]
    pub fn vaults(&self) -> Vec<&Vault> {
        let mut list: Vec<&Vault> = self.vaults.values().map(|e| &e.vault).collect();
        list.sort_by_key(|v| v.name.to_lowercase());
        list
    }

    /// Looks up a vault by id.
    pub fn vault(&self, id: VaultId) -> Result<&Vault> {
        Ok(&self.vault_entry(id)?.vault)
    }

    /// Finds a vault by case-insensitive name.
    #[must_use]
    pub fn vault_by_name(&self, name: &str) -> Option<&Vault> {
        let needle = name.trim();
        self.vaults
            .values()
            .map(|e| &e.vault)
            .find(|v| v.name.eq_ignore_ascii_case(needle))
    }

    /// Creates a vault with a fresh random key.
    pub fn create_vault(&mut self, name: &str) -> Result<VaultId> {
        let vault = Vault::new(name, Timestamp::now())?;
        let key = SymmetricKey::generate()?;
        let record = VaultRecord {
            vault,
            key: *key.expose(),
        };
        self.persist_vault(&record)?;
        Ok(record.vault.id)
    }

    /// Renames a vault.
    pub fn rename_vault(&mut self, id: VaultId, name: &str) -> Result<()> {
        let entry = self.vault_entry(id)?;
        let mut vault = entry.vault.clone();
        vault.rename(name)?;
        let record = VaultRecord {
            vault,
            key: *entry.key.expose(),
        };
        self.persist_vault(&record)
    }

    /// Deletes an empty vault (items in Recently Deleted are purged with it).
    pub fn delete_vault(&mut self, id: VaultId) -> Result<()> {
        self.vault_entry(id)?;
        if !self.items_in(id).is_empty() {
            return Err(VaultError::VaultNotEmpty);
        }
        let deleted: Vec<_> = self
            .items
            .values()
            .filter(|e| e.summary.vault_id == id)
            .map(|e| e.summary.id)
            .collect();
        self.purge(&deleted)?;
        self.db.delete_record(id.as_bytes())?;
        self.vaults.remove(&id);
        Ok(())
    }

    pub(crate) fn persist_vault(&mut self, record: &VaultRecord) -> Result<()> {
        self.write_record(
            record.vault.id.as_bytes(),
            &RecordBody::Vault(record.clone()),
        )?;
        self.insert_vault(record);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview};

    use crate::store::test_support::new_session;
    use crate::{Session, VaultError};

    fn session() -> (tempfile::TempDir, Session) {
        new_session()
    }

    #[test]
    fn new_store_has_personal_vault() {
        let (_d, s) = session();
        let names: Vec<_> = s.vaults().iter().map(|v| v.name.clone()).collect();
        assert_eq!(names, vec!["Personal"]);
    }

    #[test]
    fn create_rename_and_find_vault() {
        let (_d, mut s) = session();
        let id = s.create_vault("Work").unwrap();
        assert_eq!(s.vault(id).unwrap().name, "Work");
        s.rename_vault(id, "Job").unwrap();
        assert_eq!(s.vault_by_name("job").unwrap().id, id);
        assert!(s.vault_by_name("Work").is_none());
    }

    #[test]
    fn vaults_are_sorted_by_name() {
        let (_d, mut s) = session();
        s.create_vault("alpha").unwrap();
        s.create_vault("Zeta").unwrap();
        let names: Vec<_> = s.vaults().iter().map(|v| v.name.clone()).collect();
        assert_eq!(names, vec!["alpha", "Personal", "Zeta"]);
    }

    #[test]
    fn invalid_names_are_rejected() {
        let (_d, mut s) = session();
        assert!(matches!(s.create_vault(" "), Err(VaultError::Model(_))));
        let id = s.create_vault("A").unwrap();
        assert!(s.rename_vault(id, "").is_err());
    }

    #[test]
    fn unknown_vault_is_not_found() {
        let (_d, mut s) = session();
        let ghost = password365_model::VaultId::new();
        assert!(matches!(s.vault(ghost), Err(VaultError::NotFound("vault"))));
        assert!(matches!(
            s.rename_vault(ghost, "x"),
            Err(VaultError::NotFound(_))
        ));
        assert!(matches!(
            s.delete_vault(ghost),
            Err(VaultError::NotFound(_))
        ));
    }

    #[test]
    fn only_empty_vaults_can_be_deleted() {
        let (_d, mut s) = session();
        let id = s.create_vault("Temp").unwrap();
        s.add_item(
            id,
            ItemKind::Password,
            ItemOverview::new("x").unwrap(),
            ItemDetails::default(),
        )
        .unwrap();
        assert!(matches!(s.delete_vault(id), Err(VaultError::VaultNotEmpty)));
        let empty = s.create_vault("Empty").unwrap();
        s.delete_vault(empty).unwrap();
        assert!(s.vault(empty).is_err());
    }
}
