//! An item's replicated state: stamped fields, merged password history and
//! an optional deletion marker. Merging two states is deterministic and
//! order-independent, so every device converges on the same item.

use password365_model::password_history::PasswordHistory;
use password365_model::{Item, ItemId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::fields::{Fields, Stamps};
use crate::{Hlc, history};

/// A deletion, remembering which writes it had seen. An edit the deleting
/// device never saw brings the item back ("edit wins").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    /// When the item was deleted.
    pub at: Hlc,
    /// Field stamps at the moment of deletion.
    pub seen: Stamps,
}

impl Tombstone {
    pub(crate) fn merge(&self, other: &Self) -> Self {
        Self {
            at: self.at.max(other.at),
            seen: self.seen.max(&other.seen),
        }
    }
}

/// Replicated state of one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncItem {
    /// Item identifier (equal on every device).
    pub id: ItemId,
    /// Creation time (earliest seen).
    pub created_at: Timestamp,
    /// Stamped fields.
    pub fields: Fields,
    /// Previous passwords, merged as a set.
    pub history: PasswordHistory,
    /// Set once deleted.
    pub tombstone: Option<Tombstone>,
}

/// Result of merging two states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// The converged state.
    pub item: SyncItem,
    /// Two different passwords were set concurrently: the newer one is
    /// current, the other was kept in history. The user should review it.
    pub password_conflict: bool,
}

impl SyncItem {
    /// State of a brand-new (or first-seen) item, all fields written at `stamp`.
    #[must_use]
    pub fn capture(item: &Item, stamp: Hlc) -> Self {
        Self {
            id: item.id,
            created_at: item.created_at,
            fields: Fields::capture(item, stamp),
            history: item.details.password_history.clone(),
            tombstone: None,
        }
    }

    /// Records a local edit: changed fields get `stamp`. Returns whether anything changed.
    pub fn record(&mut self, item: &Item, stamp: Hlc) -> bool {
        debug_assert!(
            stamp > self.latest(),
            "clock must observe the state before writing"
        );
        let history = history::union(&self.history, &item.details.password_history);
        let history_changed = history != self.history;
        self.history = history;
        self.fields.record(item, stamp) | history_changed
    }

    /// Marks the item deleted at `stamp`.
    pub fn delete(&mut self, stamp: Hlc) {
        debug_assert!(
            stamp > self.latest(),
            "clock must observe the state before writing"
        );
        let tombstone = Tombstone {
            at: stamp,
            seen: self.fields.stamps(),
        };
        self.tombstone = Some(self.tombstone.map_or(tombstone, |t| t.merge(&tombstone)));
    }

    /// Undoes a deletion: rewrites every field at `stamp`, which no deletion
    /// has seen, so the item comes back on every device.
    pub fn restore(&mut self, stamp: Hlc) {
        debug_assert!(
            stamp > self.latest(),
            "clock must observe the state before writing"
        );
        self.fields.restamp(&Stamps::uniform(stamp));
    }

    /// `true` when a deletion covers every write.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.tombstone
            .is_some_and(|t| !self.fields.stamps().any_newer_than(&t.seen))
    }

    /// Newest write (field edit or deletion).
    #[must_use]
    pub fn latest(&self) -> Hlc {
        let edited = self.fields.stamps().latest();
        self.tombstone.map_or(edited, |t| edited.max(t.at))
    }

    /// The plain item. `version` is the local revision counter.
    #[must_use]
    pub fn to_item(&self, version: u64) -> Item {
        let mut item = Item::new(
            self.fields.vault_id.value,
            self.fields.kind.value,
            password365_model::ItemOverview::default(),
            password365_model::ItemDetails::default(),
            self.created_at,
        );
        item.id = self.id;
        self.fields.write_to(&mut item);
        item.details.password_history =
            history::without(&self.history, self.fields.password.value.as_ref());
        item.updated_at =
            Timestamp::from_millis(self.fields.stamps().latest().wall_ms).max(self.created_at);
        item.version = version;
        item
    }

    /// Converges two states of the same item.
    ///
    /// # Panics
    /// If the ids differ (a caller bug).
    #[must_use]
    pub fn merge(&self, other: &Self) -> Merged {
        assert_eq!(self.id, other.id, "merging different items");
        let fields = self.fields.merge(&other.fields);
        let mut history = history::union(&self.history, &other.history);
        let mut password_conflict = false;
        let loser = if fields.password.stamp == self.fields.password.stamp {
            other
        } else {
            self
        };
        // Every superseded write is kept, whatever the merge order, which makes
        // the history the same on every device (equal stamps = the same write).
        if let Some(lost) = &loser.fields.password.value
            && loser.fields.password.stamp != fields.password.stamp
        {
            password_conflict =
                fields.password.value.as_ref() != Some(lost) && !history::contains(&history, lost);
            let mut kept = PasswordHistory::default();
            kept.record(
                lost.clone(),
                Timestamp::from_millis(loser.fields.password.stamp.wall_ms),
            );
            history = history::union(&history, &kept);
        }
        let tombstone = match (self.tombstone, other.tombstone) {
            (Some(a), Some(b)) => Some(a.merge(&b)),
            (a, b) => a.or(b),
        };
        Merged {
            item: Self {
                id: self.id,
                created_at: self.created_at.min(other.created_at),
                fields,
                history,
                tombstone,
            },
            password_conflict,
        }
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview, VaultId};

    use super::*;
    use crate::DeviceId;

    fn at(wall_ms: i64, device: u8) -> Hlc {
        Hlc {
            wall_ms,
            counter: 0,
            device: DeviceId::from_bytes([device; 16]),
        }
    }

    fn item() -> Item {
        Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("Mail").unwrap().with_username("me"),
            ItemDetails::with_password("p1".into()),
            Timestamp::from_millis(1),
        )
    }

    fn edit(state: &mut SyncItem, stamp: Hlc, f: impl FnOnce(&mut Item)) {
        let mut it = state.to_item(1);
        f(&mut it);
        state.record(&it, stamp);
    }

    fn change_password(state: &mut SyncItem, stamp: Hlc, new: &str) {
        edit(state, stamp, |it| {
            it.set_password(Some(new.into()), Timestamp::from_millis(stamp.wall_ms));
        });
    }

    #[test]
    fn capture_round_trips_to_the_same_item() {
        let it = item();
        let state = SyncItem::capture(&it, at(1, 1));
        assert_eq!(state.to_item(it.version), it);
        assert!(!state.is_deleted());
    }

    #[test]
    fn record_reports_whether_anything_changed() {
        let it = item();
        let mut state = SyncItem::capture(&it, at(1, 1));
        assert!(!state.record(&it, at(2, 1)));
        let mut changed = it;
        changed
            .details
            .password_history
            .record("older".into(), Timestamp::from_millis(0));
        assert!(
            state.record(&changed, at(3, 1)),
            "history-only change counts"
        );
    }

    #[test]
    fn edits_to_different_fields_both_survive() {
        let base = SyncItem::capture(&item(), at(1, 1));
        let mut phone = base.clone();
        let mut laptop = base;
        edit(&mut phone, at(10, 1), |it| {
            it.overview.username = Some("phone".into());
        });
        edit(&mut laptop, at(11, 2), |it| {
            it.details.notes = Some("laptop".into());
        });
        let merged = phone.merge(&laptop);
        let out = merged.item.to_item(1);
        assert_eq!(out.overview.username.as_deref(), Some("phone"));
        assert_eq!(
            out.details
                .notes
                .as_ref()
                .map(|n| n.expose().to_owned())
                .as_deref(),
            Some("laptop")
        );
        assert!(!merged.password_conflict);
        assert_eq!(merged.item, laptop.merge(&phone).item);
        assert_eq!(out.updated_at, Timestamp::from_millis(11));
    }

    #[test]
    fn concurrent_passwords_keep_the_loser_in_history_and_flag_it() {
        let base = SyncItem::capture(&item(), at(1, 1));
        let mut a = base.clone();
        let mut b = base;
        change_password(&mut a, at(10, 1), "from-a");
        change_password(&mut b, at(20, 2), "from-b");
        let merged = a.merge(&b);
        assert!(merged.password_conflict);
        let out = merged.item.to_item(1);
        assert_eq!(out.details.password, Some("from-b".into()));
        assert!(history::contains(
            &out.details.password_history,
            &"from-a".into()
        ));
        assert!(history::contains(
            &out.details.password_history,
            &"p1".into()
        ));
        assert_eq!(merged.item, b.merge(&a).item);
        assert!(b.merge(&a).password_conflict);
    }

    #[test]
    fn a_plain_update_is_not_a_conflict() {
        let base = SyncItem::capture(&item(), at(1, 1));
        let mut a = base.clone();
        change_password(&mut a, at(10, 1), "p2");
        let merged = base.merge(&a);
        assert!(!merged.password_conflict);
        assert_eq!(merged.item.to_item(1).details.password, Some("p2".into()));
        assert!(!a.merge(&base).password_conflict);
    }

    #[test]
    fn clearing_a_password_never_loses_it() {
        let base = SyncItem::capture(&item(), at(1, 1));
        let mut a = base.clone();
        edit(&mut a, at(5, 1), |it| it.details.password = None);
        let merged = a.merge(&base);
        assert_eq!(merged.item.fields.password.value, None);
        assert!(history::contains(&merged.item.history, &"p1".into()));
    }

    #[test]
    fn deletion_wins_over_writes_it_had_seen() {
        let base = SyncItem::capture(&item(), at(1, 1));
        let mut deleted = base.clone();
        deleted.delete(at(5, 1));
        assert!(deleted.is_deleted());
        assert_eq!(deleted.latest(), at(5, 1));
        assert!(base.merge(&deleted).item.is_deleted());
        assert!(deleted.merge(&base).item.is_deleted());
    }

    #[test]
    fn an_unseen_edit_brings_a_deleted_item_back() {
        let base = SyncItem::capture(&item(), at(1, 1));
        let mut deleted = base.clone();
        deleted.delete(at(50, 1));
        let mut edited = base;
        edit(&mut edited, at(10, 2), |it| {
            it.overview.title = "Renamed".into();
        });
        let merged = deleted.merge(&edited).item;
        assert!(!merged.is_deleted(), "edit wins even with an older clock");
        assert_eq!(merged.fields.title.value, "Renamed");
    }

    #[test]
    fn restore_brings_an_item_back_everywhere() {
        let base = SyncItem::capture(&item(), at(1, 1));
        let mut deleted = base.clone();
        deleted.delete(at(5, 1));
        let mut restored = deleted.clone();
        restored.restore(at(6, 1));
        assert!(!restored.is_deleted());
        assert!(!deleted.merge(&restored).item.is_deleted());
        assert_eq!(restored.fields.stamps(), Stamps::uniform(at(6, 1)));
    }

    #[test]
    fn two_deletions_merge() {
        let base = SyncItem::capture(&item(), at(1, 1));
        let mut a = base.clone();
        let mut b = base;
        a.delete(at(5, 1));
        b.delete(at(6, 2));
        a.delete(at(7, 1));
        let merged = a.merge(&b).item;
        assert!(merged.is_deleted());
        assert_eq!(merged.tombstone.unwrap().at, at(7, 1));
    }

    #[test]
    #[should_panic(expected = "merging different items")]
    fn merging_different_items_is_a_bug() {
        let a = SyncItem::capture(&item(), at(1, 1));
        let b = SyncItem::capture(&item(), at(1, 1));
        let _ = a.merge(&b);
    }
}
