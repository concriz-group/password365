//! An unlocked store: holds keys in memory and caches decrypted overviews.
//!
//! Vault operations live in `session_vaults.rs`, item operations in
//! `session_items.rs`, sync in `session_sync.rs`; this file holds loading,
//! persistence helpers and account-level operations.

use std::collections::BTreeMap;

use password365_crypto::kdf::KdfParams;
use password365_crypto::key::SymmetricKey;
use password365_model::{ItemId, Vault, VaultId};

use crate::db::{Database, RecordId};
use crate::header::Header;
use crate::item_crypto;
use crate::item_summary::ItemSummary;
use crate::records::{
    self, DeviceKeysRecord, DeviceRecord, ItemRecord, ItemSyncRecord, RecordBody, VaultRecord,
};
use crate::session_peers::{DEVICE_KEYS_RECORD_ID, Peer};
use crate::store::Credentials;
use crate::{Result, VaultError};

/// A vault and its key, held while unlocked.
pub(crate) struct VaultEntry {
    pub vault: Vault,
    pub key: SymmetricKey,
}

/// An item record plus its decrypted summary and sync state.
pub(crate) struct ItemEntry {
    pub summary: ItemSummary,
    pub record: ItemRecord,
    pub sync: ItemSyncRecord,
}

impl ItemEntry {
    /// `true` while the item sits in Recently Deleted.
    pub fn is_deleted(&self) -> bool {
        matches!(
            self.sync.body,
            records::ItemSyncBody::Live { deleted: true, .. }
        )
    }
}

/// An unlocked store. Dropping it (or calling [`Session::lock`]) wipes all keys.
pub struct Session {
    pub(crate) db: Database,
    pub(crate) header: Header,
    pub(crate) storage_key: SymmetricKey,
    pub(crate) vaults: BTreeMap<VaultId, VaultEntry>,
    pub(crate) items: BTreeMap<ItemId, ItemEntry>,
    /// Purged items: only their deletion markers remain.
    pub(crate) purged: BTreeMap<ItemId, ItemSyncRecord>,
    pub(crate) device: DeviceRecord,
    pub(crate) device_keys: DeviceKeysRecord,
    pub(crate) peers: BTreeMap<password365_sync::DeviceId, Peer>,
    /// Items flagged for review on this device.
    pub(crate) review: std::collections::BTreeSet<ItemId>,
    /// Devices removed from the account.
    pub(crate) revoked: std::collections::BTreeSet<password365_sync::DeviceId>,
    /// The account's latest master password change.
    pub(crate) password: Option<records::PasswordRecord>,
}

impl Session {
    /// Loads and decrypts everything the storage key can open.
    ///
    /// Records that do not decrypt are left untouched: they may belong to a
    /// future format or another space (see `docs/ideas`), so they are never deleted.
    pub(crate) fn load(db: Database, header: Header, storage_key: SymmetricKey) -> Result<Self> {
        let mut session = Self {
            db,
            header,
            storage_key,
            vaults: BTreeMap::new(),
            items: BTreeMap::new(),
            purged: BTreeMap::new(),
            device: DeviceRecord::fresh(),
            device_keys: DeviceKeysRecord::generate()?,
            peers: BTreeMap::new(),
            review: std::collections::BTreeSet::new(),
            revoked: std::collections::BTreeSet::new(),
            password: None,
        };
        // Before anything is read or written: nothing rolled back or removed.
        session.db.check_integrity(crate::integrity::key(
            &session.storage_key,
            &session.header.store_id,
        )?)?;
        let mut item_records = Vec::new();
        let mut sync_records = BTreeMap::new();
        let mut device = None;
        let mut device_keys = None;
        let mut peers = Vec::new();
        let mut review = None;
        let mut revoked = None;
        let mut password = None;
        for (id, blob) in session.db.records()? {
            match records::open(&session.storage_key, &session.header.store_id, &id, &blob) {
                Ok(RecordBody::Vault(r)) => session.insert_vault(&r),
                Ok(RecordBody::Item(r)) => item_records.push(r),
                Ok(RecordBody::Device(d)) => device = Some(d),
                Ok(RecordBody::ItemSync(r)) => {
                    sync_records.insert(r.item_id, r);
                }
                Ok(RecordBody::DeviceKeys(k)) => device_keys = Some(k),
                Ok(RecordBody::Peer(p)) => peers.push(records::PeerRecord {
                    peer: p.peer,
                    vaults: None,
                }),
                Ok(RecordBody::PeerWithVaults(p)) => peers.push(p),
                Ok(RecordBody::Review(r)) => review = Some(r),
                Ok(RecordBody::Revoked(r)) => revoked = Some(r),
                Ok(RecordBody::Password(r)) => password = Some(r),
                Err(_) => {}
            }
        }
        if let Some(device) = device {
            session.device = device;
        } else {
            session.write_device(&session.device)?;
        }
        match device_keys {
            Some(keys) => session.device_keys = keys,
            None => session.write_record(
                &DEVICE_KEYS_RECORD_ID,
                &RecordBody::DeviceKeys(session.device_keys.clone()),
            )?,
        }
        session.load_peers(peers);
        session.load_review(review);
        session.load_revoked(revoked);
        session.load_password(password);
        let mut legacy = Vec::new();
        for record in item_records {
            let key = &session.vault_entry(record.vault_id)?.key;
            let summary = item_crypto::open_summary(key, &record)?;
            match sync_records.remove(&record.item_id) {
                Some(sync) => {
                    session.items.insert(
                        record.item_id,
                        ItemEntry {
                            summary,
                            record,
                            sync,
                        },
                    );
                }
                None => legacy.push(record),
            }
        }
        for sync in sync_records.into_values() {
            if matches!(sync.body, records::ItemSyncBody::Purged(_)) {
                session.purged.insert(sync.item_id, sync);
            }
        }
        session.catch_up_clock();
        session.adopt_legacy_items(&legacy)?;
        Ok(session)
    }

    /// Locks the session, wiping keys from memory.
    pub fn lock(self) {}

    /// Changes the master password and/or Secret Key (and optionally KDF
    /// cost). The store also gets a new storage key, every record re-encrypted
    /// under it in the same transaction: someone who knew the old password and
    /// kept an old copy of the header (a backup) cannot open later copies.
    pub fn change_credentials(&mut self, new: &Credentials, kdf: KdfParams) -> Result<()> {
        let new_key = SymmetricKey::generate()?;
        let header = self.header.rewrap(&new_key, new, kdf)?;
        let store_id = self.header.store_id;
        let new_integrity = crate::integrity::key(&new_key, &store_id)?;
        self.db.in_transaction(|| {
            for (id, blob) in self.db.records()? {
                // Records the key does not open (another format) stay as they are.
                if let Ok(moved) =
                    records::reseal(&self.storage_key, &new_key, &store_id, &id, &blob)
                {
                    self.db.put_record(&id, &moved)?;
                }
            }
            self.db.store_sum_under(&new_integrity)?;
            self.db.write_header(&header)
        })?;
        self.db.use_integrity_key(new_integrity);
        self.header = header;
        self.storage_key = new_key;
        Ok(())
    }

    /// Writes an encrypted backup of the whole store to `dest` (see
    /// [`crate::Store::restore_from`]). Nothing in it is readable without the
    /// master password and Secret Key.
    pub fn backup_to(&self, dest: &std::path::Path) -> Result<()> {
        self.db.backup_to(dest)
    }

    /// KDF parameters currently in use.
    #[must_use]
    pub fn kdf_params(&self) -> KdfParams {
        self.header.kdf
    }

    pub(crate) fn insert_vault(&mut self, record: &VaultRecord) {
        let entry = VaultEntry {
            vault: record.vault.clone(),
            key: SymmetricKey::from_bytes(record.key),
        };
        self.vaults.insert(record.vault.id, entry);
    }

    pub(crate) fn vault_entry(&self, id: VaultId) -> Result<&VaultEntry> {
        self.vaults.get(&id).ok_or(VaultError::NotFound("vault"))
    }

    /// Encrypts and writes a record body.
    pub(crate) fn write_record(&self, id: &RecordId, body: &RecordBody) -> Result<()> {
        let blob = records::seal(&self.storage_key, &self.header.store_id, id, body)?;
        self.db.put_record(id, &blob)
    }
}
