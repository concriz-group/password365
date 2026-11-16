//! Record bodies stored (encrypted) in the `records` table.

use password365_crypto::key::SymmetricKey;
use password365_model::{ItemId, Vault, VaultId};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::context::{self, StoreId};
use crate::db::RecordId;
use crate::{Result, envelope};

/// Everything that can live in a record. New variants must be appended (postcard
/// encodes the variant index).
// Decoded one at a time while loading; size differences are irrelevant.
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize)]
pub(crate) enum RecordBody {
    /// A vault and its key.
    Vault(VaultRecord),
    /// An item, encrypted again under its vault key.
    Item(ItemRecord),
    /// This install's sync identity and clock (one per store).
    Device(DeviceRecord),
    /// Sync state of one item (stamps, merged history, deletion).
    ItemSync(ItemSyncRecord),
    /// This install's pairing key and display name (one per store).
    DeviceKeys(DeviceKeysRecord),
    /// A paired device (before the vaults it was given were kept).
    Peer(LegacyPeerRecord),
    /// Items to review on this device (two passwords set at once elsewhere).
    Review(ReviewRecord),
    /// Devices removed from the account (synced to every paired device).
    Revoked(RevokedRecord),
    /// The account's latest master password change.
    Password(PasswordRecord),
    /// A paired device and the vaults it was given (replaces `Peer`).
    PeerWithVaults(PeerRecord),
}

/// The latest master password change and whether this store uses it yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PasswordRecord {
    pub latest: crate::session_password::PasswordChange,
    pub applied: bool,
}

/// Devices removed from the account; only ever grows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RevokedRecord {
    pub devices: Vec<password365_sync::DeviceId>,
}

/// Items whose concurrent password change the user has not looked at yet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReviewRecord {
    pub items: Vec<ItemId>,
}

/// This install's long-term Noise key and the name other devices see.
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub(crate) struct DeviceKeysRecord {
    pub static_private: [u8; 32],
    pub static_public: [u8; 32],
    #[zeroize(skip)]
    pub name: String,
}

/// A paired device and how far sync with it has got.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PeerRecord {
    pub peer: crate::Peer,
    /// `None` in records written before the grant was kept (`Peer` variant).
    pub vaults: Option<Vec<VaultId>>,
}

/// The original peer record (no vaults): still read, never written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LegacyPeerRecord {
    pub peer: crate::Peer,
}

/// This install's sync identity, clock and change counter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DeviceRecord {
    pub clock: password365_sync::Clock,
    /// Last change sequence number issued.
    pub seq: u64,
}

impl DeviceRecord {
    /// A new identity with a fresh clock.
    pub(crate) fn fresh() -> Self {
        Self {
            clock: password365_sync::Clock::new(password365_sync::DeviceId::new()),
            seq: 0,
        }
    }
}

/// Sync state of one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ItemSyncRecord {
    pub item_id: ItemId,
    /// Change sequence of the last write or merge that touched the item.
    pub seq: u64,
    /// Newest stamp in the state (lets the clock catch up on load).
    pub latest: password365_sync::Hlc,
    pub body: ItemSyncBody,
}

/// Live state (sealed under the vault key: it holds password history) or a purge marker.
#[allow(clippy::large_enum_variant)] // one per item; boxing buys nothing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ItemSyncBody {
    /// Item present (possibly in Recently Deleted).
    Live {
        vault_id: VaultId,
        deleted: bool,
        /// Sealed [`password365_sync::SyncMeta`].
        meta: Vec<u8>,
    },
    /// Item purged: only the deletion remains.
    Purged(password365_sync::Tombstone),
}

/// A vault together with its 256-bit key.
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub(crate) struct VaultRecord {
    #[zeroize(skip)]
    pub vault: Vault,
    pub key: [u8; 32],
}

/// An item: two halves sealed under the vault key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ItemRecord {
    pub item_id: ItemId,
    pub vault_id: VaultId,
    pub version: u64,
    /// Sealed item head (overview + metadata), see `item_crypto`.
    pub head: Vec<u8>,
    /// Sealed item details (secrets).
    pub details: Vec<u8>,
}

/// Encrypts a record body under the storage key.
pub(crate) fn seal(
    storage_key: &SymmetricKey,
    store_id: &StoreId,
    record_id: &RecordId,
    body: &RecordBody,
) -> Result<Vec<u8>> {
    envelope::seal(storage_key, body, &context::record(store_id, record_id))
}

/// Re-encrypts a sealed record from `old` to `new` (body untouched).
pub(crate) fn reseal(
    old: &SymmetricKey,
    new: &SymmetricKey,
    store_id: &StoreId,
    record_id: &RecordId,
    blob: &[u8],
) -> Result<Vec<u8>> {
    envelope::reseal(old, new, blob, &context::record(store_id, record_id))
}

/// Decrypts a record body.
pub(crate) fn open(
    storage_key: &SymmetricKey,
    store_id: &StoreId,
    record_id: &RecordId,
    blob: &[u8],
) -> Result<RecordBody> {
    envelope::open(storage_key, blob, &context::record(store_id, record_id))
}

#[cfg(test)]
mod tests {
    use password365_model::Timestamp;

    use super::*;

    #[test]
    fn vault_record_round_trip() {
        let key = SymmetricKey::generate().unwrap();
        let vault = Vault::new("Personal", Timestamp::from_millis(1)).unwrap();
        let body = RecordBody::Vault(VaultRecord {
            vault: vault.clone(),
            key: [7; 32],
        });
        let blob = seal(&key, &[1; 16], &[2; 16], &body).unwrap();
        match open(&key, &[1; 16], &[2; 16], &blob).unwrap() {
            RecordBody::Vault(r) => {
                assert_eq!(r.vault, vault);
                assert_eq!(r.key, [7; 32]);
            }
            _ => panic!("expected vault"),
        }
    }

    #[test]
    fn record_cannot_be_moved_to_another_id() {
        let key = SymmetricKey::generate().unwrap();
        let body = RecordBody::Item(ItemRecord {
            item_id: ItemId::new(),
            vault_id: VaultId::new(),
            version: 1,
            head: vec![],
            details: vec![],
        });
        let blob = seal(&key, &[1; 16], &[2; 16], &body).unwrap();
        assert!(open(&key, &[1; 16], &[3; 16], &blob).is_err());
        assert!(open(&key, &[9; 16], &[2; 16], &blob).is_err());
    }
}
