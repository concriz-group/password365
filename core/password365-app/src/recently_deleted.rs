//! Recently Deleted (as in 1Password): deleted items stay restorable for
//! [`RETENTION_DAYS`], then are purged for good on the next unlock.

use password365_model::{ItemId, Timestamp};
use serde::Serialize;

use crate::dto::ItemSummaryDto;
use crate::service::parse_id;
use crate::{AppService, Result};

/// Days a deleted item stays in Recently Deleted.
pub const RETENTION_DAYS: i64 = 30;
const RETENTION_MS: i64 = RETENTION_DAYS * 24 * 60 * 60 * 1000;

/// An item in Recently Deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeletedItemDto {
    /// Listing fields.
    #[serde(flatten)]
    pub summary: ItemSummaryDto,
    /// When it was deleted (ms since epoch).
    pub deleted_at: i64,
    /// When it will be purged for good (ms since epoch).
    pub purge_at: i64,
}

impl AppService {
    /// Items in Recently Deleted, most recently deleted first.
    pub fn deleted_items(&mut self) -> Result<Vec<DeletedItemDto>> {
        Ok(self
            .session()?
            .deleted_items()
            .into_iter()
            .map(|(summary, deleted_at)| DeletedItemDto {
                summary: ItemSummaryDto::from(summary),
                deleted_at: deleted_at.as_millis(),
                purge_at: deleted_at.as_millis().saturating_add(RETENTION_MS),
            })
            .collect())
    }

    /// Restores an item from Recently Deleted.
    pub fn restore_item(&mut self, id: &str) -> Result<()> {
        let id = parse_id::<ItemId>(id)?;
        Ok(self.session()?.restore_item(id)?)
    }

    /// Deletes an item from Recently Deleted for good.
    pub fn delete_permanently(&mut self, id: &str) -> Result<()> {
        let id = parse_id::<ItemId>(id)?;
        Ok(self.session()?.purge_item(id)?)
    }

    /// Purges items deleted more than [`RETENTION_DAYS`] before `now`.
    pub(crate) fn purge_expired_deletions(&mut self, now: Timestamp) -> Result<()> {
        let cutoff = Timestamp::from_millis(now.as_millis().saturating_sub(RETENTION_MS));
        self.session()?.purge_deleted(cutoff)?;
        // Files of purged items (or removed from items) are not needed any more.
        self.session()?.collect_unreferenced_attachments()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppError;
    use crate::service::tests::{login, with_account};

    #[test]
    fn deleted_items_can_be_listed_restored_or_deleted_for_good() {
        let mut fx = with_account();
        let keep = fx.app.create_item(&login("Keep", "p")).unwrap().summary.id;
        let drop = fx.app.create_item(&login("Drop", "p")).unwrap().summary.id;
        fx.app.delete_item(&keep).unwrap();
        fx.app.delete_item(&drop).unwrap();
        assert!(fx.app.items("", None).unwrap().is_empty());
        let deleted = fx.app.deleted_items().unwrap();
        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted[0].summary.title, "Drop");
        assert_eq!(deleted[0].purge_at - deleted[0].deleted_at, RETENTION_MS);

        fx.app.restore_item(&keep).unwrap();
        fx.app.delete_permanently(&drop).unwrap();
        assert!(fx.app.deleted_items().unwrap().is_empty());
        let titles: Vec<String> = fx
            .app
            .items("", None)
            .unwrap()
            .into_iter()
            .map(|i| i.title)
            .collect();
        assert_eq!(titles, vec!["Keep"]);
        assert_eq!(fx.app.item(&drop).unwrap_err(), AppError::NotFound("item"));
    }

    #[test]
    fn bad_ids_and_live_items_are_rejected() {
        let mut fx = with_account();
        let live = fx.app.create_item(&login("Live", "p")).unwrap().summary.id;
        assert_eq!(
            fx.app.restore_item("nope").unwrap_err().code(),
            "validation"
        );
        assert_eq!(
            fx.app.delete_permanently("nope").unwrap_err().code(),
            "validation"
        );
        assert!(fx.app.restore_item(&live).is_err());
        assert!(fx.app.delete_permanently(&live).is_err());
    }

    #[test]
    fn only_deletions_older_than_the_retention_are_purged() {
        let mut fx = with_account();
        let id = fx.app.create_item(&login("Old", "p")).unwrap().summary.id;
        fx.app.delete_item(&id).unwrap();
        let deleted_at = fx.app.deleted_items().unwrap()[0].deleted_at;
        fx.app
            .purge_expired_deletions(Timestamp::from_millis(deleted_at + RETENTION_MS))
            .unwrap();
        assert_eq!(
            fx.app.deleted_items().unwrap().len(),
            1,
            "exactly 30 days: kept"
        );
        fx.app
            .purge_expired_deletions(Timestamp::from_millis(deleted_at + RETENTION_MS + 1))
            .unwrap();
        assert!(fx.app.deleted_items().unwrap().is_empty());
    }

    #[test]
    fn needs_an_unlocked_vault() {
        let mut fx = with_account();
        fx.app.lock();
        assert_eq!(fx.app.deleted_items().unwrap_err(), AppError::Locked);
    }
}
