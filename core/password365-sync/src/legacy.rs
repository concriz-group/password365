//! Older shapes of the replicated state, as older stores keep them and
//! older devices send them: format v1 (no `archived`) and format v2 (custom
//! fields as one register, no attachments). Each converts to the current
//! shape; nothing is ever written in these shapes again.

use password365_model::{ItemId, PasswordHistory, Timestamp};
use serde::{Deserialize, Serialize};

use crate::fields::{EntryStamps, FieldsV1, FieldsV2, StampsV1, StampsV2};
use crate::{Hlc, ItemState, SyncItem, SyncMeta, Tombstone};

macro_rules! format_shapes {
    ($v:literal, $tomb:ident, $item:ident, $state:ident, $meta:ident, $fields:ident, $stamps:ident) => {
        #[doc = concat!("[`Tombstone`] in format ", $v, ".")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $tomb {
            /// When the item was deleted.
            pub at: Hlc,
            /// Field stamps at the moment of deletion.
            pub seen: $stamps,
        }

        #[doc = concat!("[`SyncItem`] in format ", $v, ".")]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $item {
            /// Item identifier.
            pub id: ItemId,
            /// Creation time.
            pub created_at: Timestamp,
            /// Stamped fields.
            pub fields: $fields,
            /// Previous passwords.
            pub history: PasswordHistory,
            /// Set once deleted.
            pub tombstone: Option<$tomb>,
        }

        #[doc = concat!("[`ItemState`] in format ", $v, ".")]
        #[allow(clippy::large_enum_variant)]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $state {
            /// The item, possibly deleted but still restorable.
            Live($item),
            /// Deleted long ago; only the marker remains.
            Purged {
                /// Item id.
                id: ItemId,
                /// The deletion.
                tombstone: $tomb,
            },
        }

        #[doc = concat!("[`SyncMeta`] in format ", $v, ".")]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $meta {
            /// Field stamps.
            pub stamps: $stamps,
            /// Full merged password history.
            pub history: PasswordHistory,
            /// A deletion that an unseen edit overrode.
            pub tombstone: Option<$tomb>,
        }
    };
}

format_shapes!(
    "v1",
    TombstoneV1,
    SyncItemV1,
    ItemStateV1,
    SyncMetaV1,
    FieldsV1,
    StampsV1
);
format_shapes!(
    "v2",
    TombstoneV2,
    SyncItemV2,
    ItemStateV2,
    SyncMetaV2,
    FieldsV2,
    StampsV2
);

/// Converts the shapes of one format into the next one's (`$from` → `$to`).
macro_rules! upgrade {
    ($tomb:ident -> $ntomb:ident, $item:ident -> $nitem:ident, $state:ident -> $nstate:ident) => {
        impl From<$tomb> for $ntomb {
            fn from(old: $tomb) -> Self {
                Self {
                    at: old.at,
                    seen: old.seen.into(),
                }
            }
        }

        impl From<$item> for $nitem {
            fn from(old: $item) -> Self {
                Self {
                    id: old.id,
                    created_at: old.created_at,
                    fields: old.fields.into(),
                    history: old.history,
                    tombstone: old.tombstone.map(Into::into),
                }
            }
        }

        impl From<$state> for $nstate {
            fn from(old: $state) -> Self {
                match old {
                    $state::Live(item) => Self::Live(item.into()),
                    $state::Purged { id, tombstone } => Self::Purged {
                        id,
                        tombstone: tombstone.into(),
                    },
                }
            }
        }
    };
}

upgrade!(TombstoneV1 -> TombstoneV2, SyncItemV1 -> SyncItemV2, ItemStateV1 -> ItemStateV2);
upgrade!(TombstoneV2 -> Tombstone, SyncItemV2 -> SyncItem, ItemStateV2 -> ItemState);

impl From<ItemStateV1> for ItemState {
    fn from(old: ItemStateV1) -> Self {
        ItemStateV2::from(old).into()
    }
}

impl From<SyncMetaV1> for SyncMetaV2 {
    fn from(old: SyncMetaV1) -> Self {
        Self {
            stamps: old.stamps.into(),
            history: old.history,
            tombstone: old.tombstone.map(Into::into),
        }
    }
}

/// Format v2 metadata has no per-entry stamps: every custom field takes the
/// custom fields' stamp when the item is rebuilt.
impl From<SyncMetaV2> for SyncMeta {
    fn from(old: SyncMetaV2) -> Self {
        Self {
            stamps: old.stamps.into(),
            entries: EntryStamps::default(),
            history: old.history,
            tombstone: old.tombstone.map(Into::into),
        }
    }
}

impl From<SyncMetaV1> for SyncMeta {
    fn from(old: SyncMetaV1) -> Self {
        SyncMetaV2::from(old).into()
    }
}

/// Converts current shapes back (tests and tools only: loses what the older
/// format did not have).
macro_rules! downgrade {
    ($tomb:ident <- $ntomb:ident, $item:ident <- $nitem:ident, $state:ident <- $nstate:ident, $stamps:ident) => {
        impl From<&$ntomb> for $tomb {
            fn from(current: &$ntomb) -> Self {
                Self {
                    at: current.at,
                    seen: current.seen.into(),
                }
            }
        }

        impl From<&$nitem> for $item {
            fn from(current: &$nitem) -> Self {
                Self {
                    id: current.id,
                    created_at: current.created_at,
                    fields: (&current.fields).into(),
                    history: current.history.clone(),
                    tombstone: current.tombstone.as_ref().map(Into::into),
                }
            }
        }

        impl From<&$nstate> for $state {
            fn from(current: &$nstate) -> Self {
                match current {
                    $nstate::Live(item) => Self::Live(item.into()),
                    $nstate::Purged { id, tombstone } => Self::Purged {
                        id: *id,
                        tombstone: tombstone.into(),
                    },
                }
            }
        }
    };
}

downgrade!(TombstoneV2 <- Tombstone, SyncItemV2 <- SyncItem, ItemStateV2 <- ItemState, StampsV2);
downgrade!(TombstoneV1 <- TombstoneV2, SyncItemV1 <- SyncItemV2, ItemStateV1 <- ItemStateV2, StampsV1);

impl From<&ItemState> for ItemStateV1 {
    fn from(current: &ItemState) -> Self {
        Self::from(&ItemStateV2::from(current))
    }
}

impl From<&SyncMeta> for SyncMetaV2 {
    fn from(current: &SyncMeta) -> Self {
        Self {
            stamps: current.stamps.into(),
            history: current.history.clone(),
            tombstone: current.tombstone.as_ref().map(Into::into),
        }
    }
}

impl From<&SyncMeta> for SyncMetaV1 {
    fn from(current: &SyncMeta) -> Self {
        let v2 = SyncMetaV2::from(current);
        Self {
            stamps: v2.stamps.into(),
            history: v2.history,
            tombstone: v2.tombstone.as_ref().map(Into::into),
        }
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{Item, ItemDetails, ItemKind, ItemOverview, VaultId};

    use super::*;
    use crate::{Clock, DeviceId};

    fn state() -> SyncItem {
        let item = Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("Mail").unwrap(),
            ItemDetails::with_password("pw".into()),
            Timestamp::from_millis(1),
        );
        SyncItem::capture(&item, Clock::new(DeviceId::new()).tick(1))
    }

    /// What a v2 device has: everything but attachments, which upgrade to
    /// none at the zero stamp.
    fn as_v2_had_it(mut item: SyncItem) -> SyncItem {
        let zero = Hlc::zero(item.fields.kind.stamp.device);
        item.fields.attachments = crate::LwwMap::empty(zero);
        if let Some(t) = item.tombstone.as_mut() {
            t.seen.attachments = Hlc::zero(t.seen.kind.device);
        }
        item
    }

    /// What a v1 device has: also no `archived`, which upgrades to `false`
    /// at the zero stamp.
    fn as_v1_had_it(item: SyncItem) -> SyncItem {
        let mut item = as_v2_had_it(item);
        let zero = Hlc::zero(item.fields.kind.stamp.device);
        item.fields.archived.stamp = zero;
        if let Some(t) = item.tombstone.as_mut() {
            t.seen.archived = Hlc::zero(t.seen.kind.device);
        }
        item
    }

    #[test]
    fn v2_states_upgrade_with_each_custom_field_at_the_old_stamp() {
        let mut live = as_v2_had_it(state());
        let mut item = live.to_item(1);
        item.details.custom_fields = vec![password365_model::CustomField {
            label: "PIN".into(),
            value: "1".into(),
            concealed: true,
        }];
        live.record(&item, Clock::new(DeviceId::new()).tick(5));
        // A v2 device kept one stamp for all custom fields.
        live.fields.custom_fields.floor = live.fields.custom_fields.latest();
        let back = ItemState::from(ItemStateV2::from(&ItemState::Live(live.clone())));
        assert_eq!(back, ItemState::Live(live.clone()));
        let meta = live.meta();
        let upgraded = SyncMeta::from(SyncMetaV2::from(&meta));
        assert_eq!(upgraded.stamps, meta.stamps);
        assert!(
            upgraded.entries.custom_fields.is_empty(),
            "v2 had no per-field stamps"
        );
        assert_eq!(
            SyncItem::from_parts(&live.to_item(1), &upgraded),
            live,
            "rebuilt with the old stamp"
        );
    }

    #[test]
    fn v1_states_upgrade_without_losing_anything_they_had() {
        let live = as_v1_had_it(state());
        let back = ItemState::from(ItemStateV1::from(&ItemState::Live(live.clone())));
        assert_eq!(back, ItemState::Live(live.clone()));
        let meta = live.meta();
        assert_eq!(SyncMeta::from(SyncMetaV1::from(&meta)), meta);
    }

    #[test]
    fn purged_and_deleted_states_upgrade_too() {
        let mut item = state();
        let at = Clock::new(DeviceId::new()).tick(9);
        item.tombstone = Some(Tombstone {
            at,
            seen: item.fields.stamps(),
        });
        let item = as_v1_had_it(item);
        let live = ItemState::Live(item.clone());
        assert_eq!(ItemState::from(ItemStateV1::from(&live)), live);
        let purged = ItemState::Purged {
            id: item.id,
            tombstone: item.tombstone.unwrap(),
        };
        assert_eq!(ItemState::from(ItemStateV1::from(&purged)), purged);
    }
}
