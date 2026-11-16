//! Items to review after a sync merged two different passwords set at the
//! same time (the newer one is current, the other is in the password history).
//! The list is local to this device.

use std::collections::BTreeSet;

use password365_model::ItemId;

use crate::Result;
use crate::db::RecordId;
use crate::records::{RecordBody, ReviewRecord};
use crate::session::Session;

/// Fixed record id (version nibble 0xf: never a `UUIDv4`).
pub(crate) const REVIEW_RECORD_ID: RecordId = {
    let mut id = [0xff; 16];
    id[15] = 0xfd;
    id
};

impl Session {
    /// Present items flagged for review.
    #[must_use]
    pub fn items_to_review(&self) -> Vec<ItemId> {
        self.review
            .iter()
            .copied()
            .filter(|id| self.items.get(id).is_some_and(|e| !e.is_deleted()))
            .collect()
    }

    /// `true` when `id` is flagged for review.
    #[must_use]
    pub fn needs_review(&self, id: ItemId) -> bool {
        self.review.contains(&id)
    }

    /// Flags items for review.
    pub fn flag_for_review(&mut self, ids: &[ItemId]) -> Result<()> {
        let mut next = self.review.clone();
        next.extend(ids.iter().copied());
        self.save_review(next)
    }

    /// The user has looked at the item.
    pub fn mark_reviewed(&mut self, id: ItemId) -> Result<()> {
        let mut next = self.review.clone();
        next.remove(&id);
        self.save_review(next)
    }

    pub(crate) fn load_review(&mut self, record: Option<ReviewRecord>) {
        self.review = record
            .map(|r| r.items.into_iter().collect())
            .unwrap_or_default();
    }

    fn save_review(&mut self, next: BTreeSet<ItemId>) -> Result<()> {
        if next == self.review {
            return Ok(());
        }
        let record = ReviewRecord {
            items: next.iter().copied().collect(),
        };
        self.write_record(&REVIEW_RECORD_ID, &RecordBody::Review(record))?;
        self.review = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use password365_crypto::kdf::KdfParams;
    use password365_model::{ItemDetails, ItemKind, ItemOverview};

    use super::*;
    use crate::Store;
    use crate::store::test_support::credentials;

    #[test]
    fn flags_persist_until_reviewed_and_skip_missing_items() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.qlvault");
        let mut s =
            Store::create(&path, &credentials("pw"), KdfParams::insecure_for_tests()).unwrap();
        let vault = s.vault_by_name("Personal").unwrap().id;
        let id = s
            .add_item(
                vault,
                ItemKind::Login,
                ItemOverview::new("Mail").unwrap(),
                ItemDetails::default(),
            )
            .unwrap();
        let gone = ItemId::new();
        s.flag_for_review(&[id, gone]).unwrap();
        s.flag_for_review(&[id]).unwrap();
        assert_eq!(
            s.items_to_review(),
            vec![id],
            "unknown items are not listed"
        );
        assert!(s.needs_review(id));
        s.lock();
        let mut s = Store::open(&path)
            .unwrap()
            .unlock(&credentials("pw"))
            .unwrap();
        assert_eq!(s.items_to_review(), vec![id]);
        s.delete_item(id).unwrap();
        assert!(
            s.items_to_review().is_empty(),
            "deleted items are not listed"
        );
        s.mark_reviewed(id).unwrap();
        assert!(!s.needs_review(id));
        assert_ne!(
            REVIEW_RECORD_ID,
            crate::session_peers::DEVICE_KEYS_RECORD_ID
        );
    }
}
