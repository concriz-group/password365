//! Where sync state lives in the store and how it is sealed.

use password365_crypto::key::SymmetricKey;
use password365_model::{ItemId, VaultId};
use password365_sync::SyncMeta;

use crate::context::{Format, ItemPart, item_part, item_part_in};
use crate::db::RecordId;
use crate::{Result, envelope};

/// Record id of the device record. Vault and item ids are `UUIDv4` (version
/// nibble 4), so ids with version nibble 0xf never collide with them.
pub(crate) const DEVICE_RECORD_ID: RecordId = [0xff; 16];

/// Record id of an item's sync state: the item id with version nibble 0xf.
pub(crate) fn item_sync_record_id(item: ItemId) -> RecordId {
    let mut id = *item.as_bytes();
    id[6] |= 0xf0;
    id
}

/// Seals sync metadata under the vault key, bound to vault, item and sequence.
pub(crate) fn seal(
    key: &SymmetricKey,
    vault: VaultId,
    item: ItemId,
    seq: u64,
    meta: &SyncMeta,
) -> Result<Vec<u8>> {
    envelope::seal(key, meta, &item_part(ItemPart::Sync, vault, item, seq))
}

/// Opens metadata sealed by [`seal`].
pub(crate) fn open(
    key: &SymmetricKey,
    vault: VaultId,
    item: ItemId,
    seq: u64,
    sealed: &[u8],
) -> Result<SyncMeta> {
    let at = |format| item_part_in(format, ItemPart::Sync, vault, item, seq);
    envelope::open(key, sealed, &at(Format::V3))
        .or_else(|_| {
            envelope::open::<password365_sync::legacy::SyncMetaV2>(key, sealed, &at(Format::V2))
                .map(Into::into)
        })
        .or_else(|_| {
            envelope::open::<password365_sync::legacy::SyncMetaV1>(key, sealed, &at(Format::V1))
                .map(Into::into)
        })
}

#[cfg(test)]
mod tests {
    use password365_model::{Item, ItemDetails, ItemKind, ItemOverview, Timestamp};
    use password365_sync::{Clock, DeviceId, SyncItem};

    use super::*;

    #[test]
    fn sync_record_ids_never_collide_with_uuid_v4_ids() {
        let item = ItemId::new();
        let id = item_sync_record_id(item);
        assert_ne!(&id, item.as_bytes());
        assert_eq!(id[6] >> 4, 0xf);
        assert_eq!(item.as_bytes()[6] >> 4, 4);
        assert_eq!(DEVICE_RECORD_ID[6] >> 4, 0xf);
        assert_ne!(item_sync_record_id(ItemId::new()), id);
    }

    #[test]
    fn meta_is_bound_to_its_place() {
        let key = SymmetricKey::generate().unwrap();
        let item = Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("x").unwrap(),
            ItemDetails::default(),
            Timestamp::from_millis(1),
        );
        let meta = SyncItem::capture(&item, Clock::new(DeviceId::new()).tick(1)).meta();
        let sealed = seal(&key, item.vault_id, item.id, 3, &meta).unwrap();
        assert_eq!(
            open(&key, item.vault_id, item.id, 3, &sealed).unwrap(),
            meta
        );
        assert!(open(&key, item.vault_id, item.id, 4, &sealed).is_err());
        assert!(open(&key, VaultId::new(), item.id, 3, &sealed).is_err());
    }

    #[test]
    fn format_v1_meta_is_still_read() {
        let key = SymmetricKey::generate().unwrap();
        let (vault, item) = (VaultId::new(), ItemId::new());
        let it = Item::new(
            vault,
            ItemKind::Login,
            ItemOverview::new("x").unwrap(),
            ItemDetails::default(),
            Timestamp::from_millis(1),
        );
        let mut meta = SyncItem::capture(&it, Clock::new(DeviceId::new()).tick(1)).meta();
        meta.stamps.archived = password365_sync::Hlc::zero(meta.stamps.kind.device);
        meta.stamps.attachments = password365_sync::Hlc::zero(meta.stamps.kind.device);
        let old = password365_sync::legacy::SyncMetaV1::from(&meta);
        let v1 = item_part_in(Format::V1, ItemPart::Sync, vault, item, 2);
        let sealed = envelope::seal(&key, &old, &v1).unwrap();
        assert_eq!(open(&key, vault, item, 2, &sealed).unwrap(), meta);
        assert!(open(&key, vault, item, 3, &sealed).is_err());
    }

    #[test]
    fn format_v2_meta_is_still_read() {
        let key = SymmetricKey::generate().unwrap();
        let (vault, item) = (VaultId::new(), ItemId::new());
        let it = Item::new(
            vault,
            ItemKind::Login,
            ItemOverview::new("x").unwrap(),
            ItemDetails::default(),
            Timestamp::from_millis(1),
        );
        let mut meta = SyncItem::capture(&it, Clock::new(DeviceId::new()).tick(1)).meta();
        meta.stamps.attachments = password365_sync::Hlc::zero(meta.stamps.kind.device);
        let old = password365_sync::legacy::SyncMetaV2::from(&meta);
        let v2 = item_part_in(Format::V2, ItemPart::Sync, vault, item, 2);
        let sealed = envelope::seal(&key, &old, &v2).unwrap();
        assert_eq!(open(&key, vault, item, 2, &sealed).unwrap(), meta);
    }
}
