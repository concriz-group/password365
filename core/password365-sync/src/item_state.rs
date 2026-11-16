//! What a device stores and exchanges per item.
//!
//! A deleted item keeps its full (encrypted) state, like 1Password's
//! "Recently Deleted", so merges stay order-independent and it can be
//! restored. After the retention period it is *purged*: values wiped, only
//! the deletion marker kept so peers delete it too.

use password365_model::password_history::PasswordHistory;
use password365_model::{Item, ItemId};
use serde::{Deserialize, Serialize};

use crate::fields::{EntryStamps, Fields, Stamps};
use crate::{Hlc, SyncItem, Tombstone};

/// Everything about a live item except its plain values, which the store
/// already keeps in the item itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncMeta {
    /// Field stamps.
    pub stamps: Stamps,
    /// Stamps of each custom field and attachment (removed ones included).
    pub entries: EntryStamps,
    /// Full merged password history (the item shows it minus the current password).
    pub history: PasswordHistory,
    /// A deletion that an unseen edit overrode.
    pub tombstone: Option<Tombstone>,
}

/// Replicated state of one item.
// Both variants are a few hundred bytes and short-lived; boxing both buys nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemState {
    /// The item, possibly deleted but still restorable.
    Live(SyncItem),
    /// Deleted long ago; secrets wiped, only the marker remains.
    Purged {
        /// Item id.
        id: ItemId,
        /// The deletion.
        tombstone: Tombstone,
    },
}

/// Result of merging two [`ItemState`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMerge {
    /// Converged state.
    pub state: ItemState,
    /// See [`crate::Merged::password_conflict`].
    pub password_conflict: bool,
}

impl SyncItem {
    /// Rebuilds the state from a stored item and its metadata.
    #[must_use]
    pub fn from_parts(item: &Item, meta: &SyncMeta) -> Self {
        let fields = Fields::from_parts(item, &meta.stamps, &meta.entries);
        Self {
            id: item.id,
            created_at: item.created_at,
            fields,
            history: meta.history.clone(),
            tombstone: meta.tombstone,
        }
    }

    /// The metadata to store next to the item.
    #[must_use]
    pub fn meta(&self) -> SyncMeta {
        SyncMeta {
            stamps: self.fields.stamps(),
            entries: self.fields.entry_stamps(),
            history: self.history.clone(),
            tombstone: self.tombstone,
        }
    }
}

impl ItemState {
    /// A live (or restorable) item.
    #[must_use]
    pub fn live(item: SyncItem) -> Self {
        Self::Live(item)
    }

    /// Item id.
    #[must_use]
    pub fn id(&self) -> ItemId {
        match self {
            Self::Live(item) => item.id,
            Self::Purged { id, .. } => *id,
        }
    }

    /// Newest write in this state.
    #[must_use]
    pub fn latest(&self) -> Hlc {
        match self {
            Self::Live(item) => item.latest(),
            Self::Purged { tombstone, .. } => tombstone.at,
        }
    }

    /// `true` when deleted (restorable or purged).
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        match self {
            Self::Live(item) => item.is_deleted(),
            Self::Purged { .. } => true,
        }
    }

    /// Deletes the item at `stamp`; it stays restorable until purged.
    #[must_use]
    pub fn deleted(self, stamp: Hlc) -> Self {
        match self {
            Self::Live(mut item) => {
                item.delete(stamp);
                Self::Live(item)
            }
            purged @ Self::Purged { .. } => purged,
        }
    }

    /// Wipes a deleted item's values, keeping only the marker.
    #[must_use]
    pub fn purged(self) -> Self {
        match self {
            Self::Live(item) => Self::settle(item),
            purged @ Self::Purged { .. } => purged,
        }
    }

    /// Converges two states of the same item.
    ///
    /// Merging live states is a CRDT (order-independent). A purged marker
    /// deletes any copy whose writes it had seen; a copy with an unseen edit
    /// comes back with the values that copy carries.
    ///
    /// # Panics
    /// If the ids differ (a caller bug).
    #[must_use]
    pub fn merge(&self, other: &Self) -> StateMerge {
        assert_eq!(self.id(), other.id(), "merging different items");
        let (state, password_conflict) = match (self, other) {
            (Self::Live(a), Self::Live(b)) => {
                let merged = a.merge(b);
                (Self::live(merged.item), merged.password_conflict)
            }
            (Self::Live(live), Self::Purged { tombstone, .. })
            | (Self::Purged { tombstone, .. }, Self::Live(live)) => {
                let mut item = live.clone();
                item.tombstone = Some(item.tombstone.map_or(*tombstone, |t| t.merge(tombstone)));
                (Self::settle(item), false)
            }
            (Self::Purged { id, tombstone: a }, Self::Purged { tombstone: b, .. }) => (
                Self::Purged {
                    id: *id,
                    tombstone: a.merge(b),
                },
                false,
            ),
        };
        StateMerge {
            state,
            password_conflict,
        }
    }

    fn settle(item: SyncItem) -> Self {
        match item.tombstone {
            Some(tombstone) if item.is_deleted() => Self::Purged {
                id: item.id,
                tombstone,
            },
            _ => Self::live(item),
        }
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview, Timestamp, VaultId};

    use super::*;
    use crate::DeviceId;

    fn at(wall_ms: i64, device: u8) -> Hlc {
        Hlc {
            wall_ms,
            counter: 0,
            device: DeviceId::from_bytes([device; 16]),
        }
    }

    fn live() -> SyncItem {
        let item = Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("Mail").unwrap(),
            ItemDetails::with_password("p1".into()),
            Timestamp::from_millis(1),
        );
        SyncItem::capture(&item, at(1, 1))
    }

    fn rename(state: &SyncItem, stamp: Hlc) -> SyncItem {
        let mut out = state.clone();
        let mut item = out.to_item(1);
        item.overview.title = "Renamed".into();
        out.record(&item, stamp);
        out
    }

    #[test]
    fn parts_round_trip() {
        let mut state = rename(&live(), at(5, 2));
        state
            .history
            .record("old".into(), Timestamp::from_millis(3));
        let item = state.to_item(4);
        assert_eq!(SyncItem::from_parts(&item, &state.meta()), state);
    }

    #[test]
    fn deleted_items_stay_restorable_until_purged() {
        let state = ItemState::live(live());
        let id = state.id();
        assert!(!state.is_deleted());
        assert_eq!(state.clone().purged(), state, "live items are never purged");
        let deleted = state.deleted(at(9, 1));
        assert!(matches!(&deleted, ItemState::Live(item) if item.is_deleted()));
        assert!(deleted.is_deleted());
        let purged = deleted.purged();
        assert!(matches!(purged, ItemState::Purged { id: i, .. } if i == id));
        assert!(purged.is_deleted());
        assert_eq!(purged.latest(), at(9, 1));
        assert_eq!(purged.clone().deleted(at(10, 1)), purged);
        assert_eq!(purged.clone().purged(), purged);
    }

    #[test]
    fn a_purge_deletes_copies_it_had_seen() {
        let base = live();
        let purged = ItemState::live(base.clone()).deleted(at(9, 1)).purged();
        for merged in [
            purged.merge(&ItemState::live(base.clone())),
            ItemState::live(base).merge(&purged),
        ] {
            assert_eq!(merged.state, purged);
            assert!(!merged.password_conflict);
        }
    }

    #[test]
    fn an_unseen_edit_survives_a_purge() {
        let base = live();
        let purged = ItemState::live(base.clone()).deleted(at(50, 1)).purged();
        let edited = ItemState::live(rename(&base, at(10, 2)));
        let merged = purged.merge(&edited).state;
        let ItemState::Live(item) = &merged else {
            panic!("expected live")
        };
        assert_eq!(item.fields.title.value, "Renamed");
        assert!(!merged.is_deleted());
        assert_eq!(edited.merge(&purged).state, merged);
        assert_eq!(merged.latest(), at(50, 1));
    }

    #[test]
    fn live_merges_report_conflicts() {
        let base = live();
        let change = |stamp: Hlc, pw: &str| {
            let mut s = base.clone();
            let mut item = s.to_item(1);
            item.set_password(Some(pw.into()), Timestamp::from_millis(stamp.wall_ms));
            s.record(&item, stamp);
            ItemState::live(s)
        };
        assert!(
            change(at(5, 1), "a")
                .merge(&change(at(6, 2), "b"))
                .password_conflict
        );
    }

    #[test]
    fn purges_merge() {
        let base = live();
        let a = ItemState::live(base.clone()).deleted(at(5, 1)).purged();
        let b = ItemState::live(base).deleted(at(6, 2)).purged();
        let merged = a.merge(&b).state;
        assert!(matches!(merged, ItemState::Purged { tombstone, .. } if tombstone.at == at(6, 2)));
    }

    #[test]
    #[should_panic(expected = "merging different items")]
    fn different_items_are_a_bug() {
        let _ = ItemState::live(live()).merge(&ItemState::live(live()));
    }
}
