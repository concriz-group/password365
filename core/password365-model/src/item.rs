//! A vault item (login, password, secure note …).

use password365_crypto::secret::SecretString;
use serde::{Deserialize, Serialize};

use crate::{ItemDetails, ItemId, ItemKind, ItemOverview, Timestamp, VaultId};

/// A complete item. Encrypted as a whole by `password365-vault`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Unique identifier.
    pub id: ItemId,
    /// Vault the item belongs to.
    pub vault_id: VaultId,
    /// Category.
    pub kind: ItemKind,
    /// Listing / search fields.
    pub overview: ItemOverview,
    /// Secret fields.
    pub details: ItemDetails,
    /// Creation time.
    pub created_at: Timestamp,
    /// Last modification time.
    pub updated_at: Timestamp,
    /// Monotonic version, incremented on every change (used by sync).
    pub version: u64,
}

impl Item {
    /// Creates a new item at version 1.
    #[must_use]
    pub fn new(
        vault_id: VaultId,
        kind: ItemKind,
        overview: ItemOverview,
        details: ItemDetails,
        now: Timestamp,
    ) -> Self {
        Self {
            id: ItemId::new(),
            vault_id,
            kind,
            overview,
            details,
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }

    /// Marks the item as modified.
    pub fn touch(&mut self, now: Timestamp) {
        self.updated_at = now.max(self.updated_at);
        self.version += 1;
    }

    /// Changes the password (keeping history) and bumps the version if it changed.
    pub fn set_password(&mut self, password: Option<SecretString>, now: Timestamp) -> bool {
        let changed = self.details.change_password(password, now);
        if changed {
            self.touch(now);
        }
        changed
    }

    /// Replaces the overview and bumps the version.
    pub fn set_overview(&mut self, overview: ItemOverview, now: Timestamp) {
        self.overview = overview;
        self.touch(now);
    }

    /// Moves the item to another vault.
    pub fn move_to(&mut self, vault_id: VaultId, now: Timestamp) {
        if self.vault_id != vault_id {
            self.vault_id = vault_id;
            self.touch(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(now: i64) -> Item {
        Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("Mail").unwrap(),
            ItemDetails::with_password("p1".into()),
            Timestamp::from_millis(now),
        )
    }

    #[test]
    fn new_item_starts_at_version_one() {
        let item = sample(10);
        assert_eq!(item.version, 1);
        assert_eq!(item.created_at, item.updated_at);
    }

    #[test]
    fn set_password_bumps_version_and_keeps_history() {
        let mut item = sample(10);
        assert!(item.set_password(Some("p2".into()), Timestamp::from_millis(20)));
        assert_eq!(item.version, 2);
        assert_eq!(item.updated_at, Timestamp::from_millis(20));
        assert_eq!(item.details.password_history.len(), 1);
    }

    #[test]
    fn unchanged_password_does_not_bump_version() {
        let mut item = sample(10);
        assert!(!item.set_password(Some("p1".into()), Timestamp::from_millis(20)));
        assert_eq!(item.version, 1);
    }

    #[test]
    fn touch_never_moves_time_backwards() {
        let mut item = sample(100);
        item.touch(Timestamp::from_millis(50));
        assert_eq!(item.updated_at, Timestamp::from_millis(100));
        assert_eq!(item.version, 2);
    }

    #[test]
    fn set_overview_bumps_version() {
        let mut item = sample(1);
        item.set_overview(ItemOverview::new("New").unwrap(), Timestamp::from_millis(2));
        assert_eq!(item.overview.title, "New");
        assert_eq!(item.version, 2);
    }

    #[test]
    fn move_to_other_vault() {
        let mut item = sample(1);
        let target = VaultId::new();
        item.move_to(target, Timestamp::from_millis(2));
        assert_eq!(item.vault_id, target);
        assert_eq!(item.version, 2);
        item.move_to(target, Timestamp::from_millis(3));
        assert_eq!(item.version, 2, "same vault is a no-op");
    }
}
