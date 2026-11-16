//! Lightweight item view used for listing and search (no secrets).

use password365_model::{Item, ItemId, ItemKind, ItemOverview, Timestamp, VaultId};
use serde::{Deserialize, Serialize};

/// Non-secret part of an item, kept decrypted in memory while unlocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemSummary {
    /// Item id.
    pub id: ItemId,
    /// Owning vault.
    pub vault_id: VaultId,
    /// Category.
    pub kind: ItemKind,
    /// Title, username, URLs, tags.
    pub overview: ItemOverview,
    /// Creation time.
    pub created_at: Timestamp,
    /// Last change.
    pub updated_at: Timestamp,
    /// Version counter.
    pub version: u64,
}

impl From<&Item> for ItemSummary {
    fn from(item: &Item) -> Self {
        Self {
            id: item.id,
            vault_id: item.vault_id,
            kind: item.kind,
            overview: item.overview.clone(),
            created_at: item.created_at,
            updated_at: item.updated_at,
            version: item.version,
        }
    }
}

#[cfg(test)]
mod tests {
    use password365_model::ItemDetails;

    use super::*;

    #[test]
    fn copies_non_secret_fields() {
        let item = Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("Bank").unwrap(),
            ItemDetails::with_password("secret".into()),
            Timestamp::from_millis(3),
        );
        let s = ItemSummary::from(&item);
        assert_eq!(
            (s.id, s.vault_id, s.kind, s.version),
            (item.id, item.vault_id, item.kind, 1)
        );
        assert_eq!(s.overview.title, "Bank");
        assert_eq!(s.created_at, Timestamp::from_millis(3));
        assert!(!format!("{s:?}").contains("secret"));
    }
}
