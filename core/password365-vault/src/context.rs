//! Associated-data contexts. Each ciphertext is bound to exactly where it belongs.

use password365_crypto::aad::Aad;
use password365_crypto::kdf::KdfParams;
use password365_model::{ItemId, VaultId};

/// Store identifier (random, 16 bytes).
pub(crate) type StoreId = [u8; 16];

/// AAD for the wrapped account keyset. Binds salt and KDF parameters so a
/// tampered header (e.g. downgraded Argon2 cost) fails to decrypt.
pub(crate) fn keyset(store_id: &StoreId, salt: &[u8], params: KdfParams) -> Aad {
    Aad::new("quillock/v1/keyset")
        .bytes(store_id)
        .bytes(salt)
        .u64(params.memory_kib.into())
        .u64(params.iterations.into())
        .u64(params.parallelism.into())
}

/// AAD for one record row.
pub(crate) fn record(store_id: &StoreId, record_id: &[u8; 16]) -> Aad {
    Aad::new("quillock/v1/record")
        .bytes(store_id)
        .bytes(record_id)
}

/// Which half of an item is sealed.
#[derive(Clone, Copy)]
pub(crate) enum ItemPart {
    /// Overview and metadata.
    Head,
    /// Secret details.
    Details,
    /// Sync metadata (stamps, merged password history).
    Sync,
}

/// Item record format. The label in the associated data says which shape
/// is inside, so a record is never read with the wrong one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Format {
    /// The original shape (read only; upgraded when read).
    V1,
    /// Adds `archived` to the head and the sync stamps.
    V2,
    /// Attachments in the details; per-entry stamps for custom fields and
    /// attachments in the sync state. The head is unchanged since v2.
    V3,
}

/// AAD for one half of an item in the current format, bound to vault, item
/// and version so halves from different versions or items cannot be mixed.
pub(crate) fn item_part(part: ItemPart, vault: VaultId, item: ItemId, version: u64) -> Aad {
    item_part_in(Format::V3, part, vault, item, version)
}

/// [`item_part`] for a given format. The details never changed shape and
/// keep their v1 label.
pub(crate) fn item_part_in(
    format: Format,
    part: ItemPart,
    vault: VaultId,
    item: ItemId,
    version: u64,
) -> Aad {
    let label = match (part, format) {
        (ItemPart::Head, Format::V1) => "quillock/v1/item-head",
        (ItemPart::Head, Format::V2 | Format::V3) => "password365/v2/item-head",
        (ItemPart::Details, Format::V1 | Format::V2) => "quillock/v1/item-details",
        (ItemPart::Details, Format::V3) => "password365/v3/item-details",
        (ItemPart::Sync, Format::V1) => "quillock/v1/item-sync",
        (ItemPart::Sync, Format::V2) => "password365/v2/item-sync",
        (ItemPart::Sync, Format::V3) => "password365/v3/item-sync",
    };
    Aad::new(label)
        .bytes(vault.as_bytes())
        .bytes(item.as_bytes())
        .u64(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyset_depends_on_params() {
        let a = keyset(&[0; 16], &[1; 32], KdfParams::DESKTOP);
        let b = keyset(&[0; 16], &[1; 32], KdfParams::MINIMUM);
        assert_ne!(a, b);
    }

    #[test]
    fn record_depends_on_ids() {
        assert_ne!(record(&[0; 16], &[1; 16]), record(&[0; 16], &[2; 16]));
        assert_ne!(record(&[0; 16], &[1; 16]), record(&[9; 16], &[1; 16]));
    }

    #[test]
    fn item_parts_differ() {
        let (v, i) = (VaultId::new(), ItemId::new());
        assert_ne!(
            item_part(ItemPart::Head, v, i, 1),
            item_part(ItemPart::Details, v, i, 1)
        );
        assert_ne!(
            item_part(ItemPart::Head, v, i, 1),
            item_part(ItemPart::Head, v, i, 2)
        );
        assert_ne!(
            item_part(ItemPart::Head, v, i, 1),
            item_part(ItemPart::Head, VaultId::new(), i, 1)
        );
        assert_ne!(
            item_part(ItemPart::Details, v, i, 1),
            item_part(ItemPart::Sync, v, i, 1)
        );
    }
}
