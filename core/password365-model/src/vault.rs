//! Vault metadata (the encryption key is managed by `password365-vault`).

use serde::{Deserialize, Serialize};

use crate::error::required_text;
use crate::{Result, Timestamp, VaultId};

/// Maximum vault name length in characters.
pub const MAX_VAULT_NAME: usize = 64;

/// A named container of items, e.g. "Personal" or "Work".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vault {
    /// Unique identifier.
    pub id: VaultId,
    /// Display name.
    pub name: String,
    /// Creation time.
    pub created_at: Timestamp,
}

impl Vault {
    /// Creates a vault with a validated name.
    pub fn new(name: &str, now: Timestamp) -> Result<Self> {
        Ok(Self {
            id: VaultId::new(),
            name: required_text(name, "vault name", MAX_VAULT_NAME)?,
            created_at: now,
        })
    }

    /// Renames the vault, validating the new name.
    pub fn rename(&mut self, name: &str) -> Result<()> {
        self.name = required_text(name, "vault name", MAX_VAULT_NAME)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelError;

    #[test]
    fn new_trims_name() {
        let v = Vault::new("  Work ", Timestamp::from_millis(5)).unwrap();
        assert_eq!(v.name, "Work");
        assert_eq!(v.created_at, Timestamp::from_millis(5));
    }

    #[test]
    fn new_rejects_empty_name() {
        assert_eq!(
            Vault::new(" ", Timestamp::now()),
            Err(ModelError::Empty("vault name"))
        );
    }

    #[test]
    fn rename_validates() {
        let mut v = Vault::new("A", Timestamp::now()).unwrap();
        v.rename("B").unwrap();
        assert_eq!(v.name, "B");
        assert!(v.rename(&"x".repeat(MAX_VAULT_NAME + 1)).is_err());
        assert_eq!(v.name, "B");
    }
}
