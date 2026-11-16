//! Sealing items under their vault key: overview ("head") and secrets
//! ("details") are encrypted separately so listing never decrypts secrets.

use password365_crypto::key::SymmetricKey;
use password365_model::{Item, ItemDetails, ItemKind, ItemOverview, Timestamp};
use serde::{Deserialize, Serialize};

use crate::context::{Format, ItemPart, item_part, item_part_in};
use crate::item_summary::ItemSummary;
use crate::records::ItemRecord;
use crate::{Result, VaultError, envelope};

/// Encrypted "head" half of an item.
#[derive(Serialize, Deserialize)]
struct ItemHead {
    kind: ItemKind,
    overview: ItemOverview,
    created_at: Timestamp,
    updated_at: Timestamp,
}

/// [`ItemOverview`] in format v1 (before `archived`).
#[derive(Serialize, Deserialize)]
struct OverviewV1 {
    title: String,
    username: Option<String>,
    urls: Vec<String>,
    tags: Vec<String>,
    favorite: bool,
}

/// [`ItemHead`] in format v1.
#[derive(Serialize, Deserialize)]
struct ItemHeadV1 {
    kind: ItemKind,
    overview: OverviewV1,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl From<ItemHeadV1> for ItemHead {
    fn from(old: ItemHeadV1) -> Self {
        let o = old.overview;
        Self {
            kind: old.kind,
            overview: ItemOverview {
                title: o.title,
                username: o.username,
                urls: o.urls,
                tags: o.tags,
                favorite: o.favorite,
                archived: false,
            },
            created_at: old.created_at,
            updated_at: old.updated_at,
        }
    }
}

/// Opens a head in the current format, else in format v1.
fn open_head(vault_key: &SymmetricKey, record: &ItemRecord) -> Result<ItemHead> {
    let at = |format| {
        item_part_in(
            format,
            ItemPart::Head,
            record.vault_id,
            record.item_id,
            record.version,
        )
    };
    envelope::open(vault_key, &record.head, &at(Format::V2))
        .or_else(|_| {
            envelope::open::<ItemHeadV1>(vault_key, &record.head, &at(Format::V1)).map(Into::into)
        })
        .map_err(|_| VaultError::Corrupt("item head"))
}

/// Encrypts `item` under its vault key.
pub(crate) fn seal(vault_key: &SymmetricKey, item: &Item) -> Result<ItemRecord> {
    let head = ItemHead {
        kind: item.kind,
        overview: item.overview.clone(),
        created_at: item.created_at,
        updated_at: item.updated_at,
    };
    let aad = |part| item_part(part, item.vault_id, item.id, item.version);
    Ok(ItemRecord {
        item_id: item.id,
        vault_id: item.vault_id,
        version: item.version,
        head: envelope::seal(vault_key, &head, &aad(ItemPart::Head))?,
        details: envelope::seal(vault_key, &item.details, &aad(ItemPart::Details))?,
    })
}

/// Decrypts only the non-secret half.
pub(crate) fn open_summary(vault_key: &SymmetricKey, record: &ItemRecord) -> Result<ItemSummary> {
    let head = open_head(vault_key, record)?;
    Ok(ItemSummary {
        id: record.item_id,
        vault_id: record.vault_id,
        kind: head.kind,
        overview: head.overview,
        created_at: head.created_at,
        updated_at: head.updated_at,
        version: record.version,
    })
}

/// Decrypts the full item (including secrets).
pub(crate) fn open_item(vault_key: &SymmetricKey, record: &ItemRecord) -> Result<Item> {
    let summary = open_summary(vault_key, record)?;
    let at = |format| {
        item_part_in(
            format,
            ItemPart::Details,
            record.vault_id,
            record.item_id,
            record.version,
        )
    };
    let details: ItemDetails = envelope::open(vault_key, &record.details, &at(Format::V3))
        .or_else(|_| {
            envelope::open::<password365_model::ItemDetailsV1>(
                vault_key,
                &record.details,
                &at(Format::V1),
            )
            .map(Into::into)
        })
        .map_err(|_| VaultError::Corrupt("item details"))?;
    Ok(Item {
        id: summary.id,
        vault_id: summary.vault_id,
        kind: summary.kind,
        overview: summary.overview,
        details,
        created_at: summary.created_at,
        updated_at: summary.updated_at,
        version: summary.version,
    })
}

#[cfg(test)]
mod tests {
    use password365_model::VaultId;

    use super::*;

    fn item() -> Item {
        Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("Mail").unwrap().with_username("me"),
            ItemDetails::with_password("pw".into()),
            Timestamp::from_millis(5),
        )
    }

    #[test]
    fn round_trip_full_item() {
        let key = SymmetricKey::generate().unwrap();
        let original = item();
        let record = seal(&key, &original).unwrap();
        assert_eq!(open_item(&key, &record).unwrap(), original);
    }

    #[test]
    fn summary_matches_item() {
        let key = SymmetricKey::generate().unwrap();
        let original = item();
        let summary = open_summary(&key, &seal(&key, &original).unwrap()).unwrap();
        assert_eq!(summary, ItemSummary::from(&original));
    }

    #[test]
    fn halves_from_different_versions_cannot_be_mixed() {
        let key = SymmetricKey::generate().unwrap();
        let mut v1 = item();
        let r1 = seal(&key, &v1).unwrap();
        v1.set_password(Some("new".into()), Timestamp::from_millis(6));
        let mut r2 = seal(&key, &v1).unwrap();
        r2.details = r1.details; // old secrets with new head
        assert!(matches!(
            open_item(&key, &r2),
            Err(VaultError::Corrupt("item details"))
        ));
    }

    #[test]
    fn tampered_version_is_detected() {
        let key = SymmetricKey::generate().unwrap();
        let mut record = seal(&key, &item()).unwrap();
        record.version = 7;
        assert!(matches!(
            open_summary(&key, &record),
            Err(VaultError::Corrupt("item head"))
        ));
    }

    #[test]
    fn a_format_v1_head_is_read_and_not_archived() {
        let key = SymmetricKey::generate().unwrap();
        let original = item();
        let mut record = seal(&key, &original).unwrap();
        let old = ItemHeadV1 {
            kind: original.kind,
            overview: OverviewV1 {
                title: "Mail".into(),
                username: Some("me".into()),
                urls: vec![],
                tags: vec!["t".into()],
                favorite: true,
            },
            created_at: original.created_at,
            updated_at: original.updated_at,
        };
        let v1 = item_part_in(
            Format::V1,
            ItemPart::Head,
            original.vault_id,
            original.id,
            original.version,
        );
        record.head = envelope::seal(&key, &old, &v1).unwrap();
        let read = open_item(&key, &record).unwrap();
        assert_eq!(read.overview.tags, ["t"]);
        assert!(read.overview.favorite && !read.overview.archived);
        assert_eq!(read.details, original.details);
    }

    #[test]
    fn an_archived_item_round_trips() {
        let key = SymmetricKey::generate().unwrap();
        let mut original = item();
        original.overview.archived = true;
        assert!(
            open_summary(&key, &seal(&key, &original).unwrap())
                .unwrap()
                .overview
                .archived
        );
    }

    #[test]
    fn wrong_vault_key_fails() {
        let record = seal(&SymmetricKey::generate().unwrap(), &item()).unwrap();
        assert!(open_item(&SymmetricKey::generate().unwrap(), &record).is_err());
    }

    #[test]
    fn details_from_before_attachments_are_read() {
        let key = SymmetricKey::generate().unwrap();
        let item = Item::new(
            password365_model::VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("x").unwrap(),
            ItemDetails::with_password("pw".into()),
            Timestamp::from_millis(1),
        );
        let mut record = seal(&key, &item).unwrap();
        let old = password365_model::ItemDetailsV1::from(&item.details);
        let v1 = item_part_in(
            Format::V1,
            ItemPart::Details,
            item.vault_id,
            item.id,
            item.version,
        );
        record.details = envelope::seal(&key, &old, &v1).unwrap();
        assert_eq!(open_item(&key, &record).unwrap().details, item.details);
    }
}
