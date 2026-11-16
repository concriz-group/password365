//! Sync state of an unlocked [`Session`]: every write goes through
//! [`Session::commit_states`], which stamps, seals and persists items with
//! their sync metadata atomically and bumps the change sequence.

use password365_model::{ItemId, Timestamp};
use password365_sync::{DeviceId, Hlc, ItemState, SyncItem};

use crate::item_crypto;
use crate::item_summary::ItemSummary;
use crate::records::{
    DeviceKeysRecord, DeviceRecord, ItemRecord, ItemSyncBody, ItemSyncRecord, RecordBody,
};
use crate::session::{ItemEntry, Session};
use crate::session_peers::DEVICE_KEYS_RECORD_ID;
use crate::sync_meta::{self, DEVICE_RECORD_ID, item_sync_record_id};
use crate::{Result, VaultError};

/// What applying a peer's changes did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Items created, changed, deleted or restored here.
    pub changed: Vec<ItemId>,
    /// Items where two different passwords were set concurrently (newest kept,
    /// the other moved to the password history): ask the user to review.
    pub conflicts: Vec<ItemId>,
    /// Items in vaults this device does not have.
    pub skipped: usize,
    /// The master password was changed on another device (see
    /// `Session::pending_password_change`).
    pub password_changed: bool,
    /// Attachment files received (their bytes).
    pub files: usize,
}

/// A state to persist and the local revision number to give its item.
pub(crate) struct Pending {
    pub state: ItemState,
    pub version: u64,
}

impl Session {
    /// This install's sync identity.
    #[must_use]
    pub fn device_id(&self) -> DeviceId {
        self.device.clock.device()
    }

    /// Sequence number of the latest local change (0 = none yet).
    #[must_use]
    pub fn sync_seq(&self) -> u64 {
        self.device.seq
    }

    /// States of every item changed after sequence `since` (`None` = all),
    /// to send to a peer. Deleted and purged items are included.
    pub fn changes_since(&self, since: Option<u64>) -> Result<Vec<ItemState>> {
        let newer = |seq: u64| since.is_none_or(|s| seq > s);
        let mut out = Vec::new();
        for (id, entry) in &self.items {
            if newer(entry.sync.seq) {
                out.push(ItemState::live(self.sync_item(*id)?));
            }
        }
        for sync in self.purged.values() {
            if newer(sync.seq)
                && let ItemSyncBody::Purged(tombstone) = sync.body
            {
                out.push(ItemState::Purged {
                    id: sync.item_id,
                    tombstone,
                });
            }
        }
        Ok(out)
    }

    /// Merges states received from a peer, atomically.
    pub fn apply_changes(&mut self, incoming: &[ItemState]) -> Result<ApplyReport> {
        let now = Timestamp::now().as_millis();
        let mut report = ApplyReport::default();
        let mut pending = Vec::new();
        for remote in incoming {
            self.device.clock.observe(remote.latest(), now);
            let id = remote.id();
            let (merged, conflict) = match self.state_of(id)? {
                Some(local) => {
                    let merged = local.merge(remote);
                    if merged.state == local {
                        continue;
                    }
                    (merged.state, merged.password_conflict)
                }
                None => (remote.clone(), false),
            };
            if let ItemState::Live(item) = &merged
                && self.vault_entry(item.fields.vault_id.value).is_err()
            {
                report.skipped += 1;
                continue;
            }
            report.changed.push(id);
            if conflict {
                report.conflicts.push(id);
            }
            let version = self.items.get(&id).map_or(1, |e| e.record.version + 1);
            pending.push(Pending {
                state: merged,
                version,
            });
        }
        self.commit_states(pending)?;
        self.flag_for_review(&report.conflicts)?;
        Ok(report)
    }

    /// Gives this store a new device identity and pairing key and forgets
    /// its peers. After restoring a backup on another computer, two installs
    /// must never share an identity, and the peers were paired with the original.
    pub fn reset_device_identity(&mut self) -> Result<()> {
        let mut device = DeviceRecord::fresh();
        device
            .clock
            .observe(self.device.clock.last(), Timestamp::now().as_millis());
        device.seq = self.device.seq;
        let mut keys = DeviceKeysRecord::generate()?;
        keys.name.clone_from(&self.device_keys.name);
        self.db.in_transaction(|| {
            self.write_device(&device)?;
            self.write_record(
                &DEVICE_KEYS_RECORD_ID,
                &RecordBody::DeviceKeys(keys.clone()),
            )
        })?;
        self.device = device;
        self.device_keys = keys;
        let peers: Vec<DeviceId> = self.peers.keys().copied().collect();
        for peer in peers {
            self.remove_peer(peer)?;
        }
        Ok(())
    }

    /// A new stamp for a local change.
    pub(crate) fn next_stamp(&mut self) -> Hlc {
        self.device.clock.tick(Timestamp::now().as_millis())
    }

    /// The replicated state of a present (live or recently deleted) item.
    pub(crate) fn sync_item(&self, id: ItemId) -> Result<SyncItem> {
        let entry = self.items.get(&id).ok_or(VaultError::NotFound("item"))?;
        let ItemSyncBody::Live { vault_id, meta, .. } = &entry.sync.body else {
            return Err(VaultError::Corrupt("item sync state"));
        };
        let key = &self.vault_entry(*vault_id)?.key;
        let item = item_crypto::open_item(key, &entry.record)?;
        let meta = sync_meta::open(key, *vault_id, id, entry.sync.seq, meta)?;
        Ok(SyncItem::from_parts(&item, &meta))
    }

    fn state_of(&self, id: ItemId) -> Result<Option<ItemState>> {
        if self.items.contains_key(&id) {
            return Ok(Some(ItemState::live(self.sync_item(id)?)));
        }
        Ok(self.purged.get(&id).and_then(|sync| match sync.body {
            ItemSyncBody::Purged(tombstone) => Some(ItemState::Purged { id, tombstone }),
            ItemSyncBody::Live { .. } => None,
        }))
    }

    /// Persists states atomically, each with a new change sequence number.
    pub(crate) fn commit_states(&mut self, pending: Vec<Pending>) -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        let mut device = self.device.clone();
        let mut writes = Vec::with_capacity(pending.len());
        for Pending { state, version } in pending {
            device.seq += 1;
            writes.push(self.prepare(state, version, device.seq)?);
        }
        self.db.in_transaction(|| {
            for write in &writes {
                match write {
                    Prepared::Live(entry) => {
                        self.write_record(
                            entry.record.item_id.as_bytes(),
                            &RecordBody::Item(entry.record.clone()),
                        )?;
                        self.write_sync(&entry.sync)?;
                    }
                    Prepared::Purged(sync) => {
                        self.db.delete_record(sync.item_id.as_bytes())?;
                        self.write_sync(sync)?;
                    }
                }
            }
            self.write_device(&device)
        })?;
        self.device = device;
        for write in writes {
            match write {
                Prepared::Live(entry) => {
                    self.purged.remove(&entry.summary.id);
                    self.items.insert(entry.summary.id, entry);
                }
                Prepared::Purged(sync) => {
                    self.items.remove(&sync.item_id);
                    self.purged.insert(sync.item_id, sync);
                }
            }
        }
        Ok(())
    }

    fn prepare(&self, state: ItemState, version: u64, seq: u64) -> Result<Prepared> {
        let latest = state.latest();
        match state {
            ItemState::Live(sync_item) => {
                let item = sync_item.to_item(version);
                let key = &self.vault_entry(item.vault_id)?.key;
                let record: ItemRecord = item_crypto::seal(key, &item)?;
                let meta = sync_meta::seal(key, item.vault_id, item.id, seq, &sync_item.meta())?;
                let sync = ItemSyncRecord {
                    item_id: item.id,
                    seq,
                    latest,
                    body: ItemSyncBody::Live {
                        vault_id: item.vault_id,
                        deleted: sync_item.is_deleted(),
                        meta,
                    },
                };
                Ok(Prepared::Live(ItemEntry {
                    summary: ItemSummary::from(&item),
                    record,
                    sync,
                }))
            }
            ItemState::Purged { id, tombstone } => Ok(Prepared::Purged(ItemSyncRecord {
                item_id: id,
                seq,
                latest,
                body: ItemSyncBody::Purged(tombstone),
            })),
        }
    }

    /// Moves the clock past every stamp in the store (in case the device
    /// record is older than the data, e.g. after a crash).
    pub(crate) fn catch_up_clock(&mut self) {
        let now = Timestamp::now().as_millis();
        let stamps: Vec<Hlc> = self
            .items
            .values()
            .map(|e| e.sync.latest)
            .chain(self.purged.values().map(|s| s.latest))
            .collect();
        for stamp in stamps {
            self.device.clock.observe(stamp, now);
        }
    }

    /// Gives items written before sync existed their first sync state,
    /// stamped at their last update by this device.
    pub(crate) fn adopt_legacy_items(&mut self, legacy: &[ItemRecord]) -> Result<()> {
        let device = self.device_id();
        let mut pending = Vec::with_capacity(legacy.len());
        for record in legacy {
            let item = item_crypto::open_item(&self.vault_entry(record.vault_id)?.key, record)?;
            let stamp = Hlc {
                wall_ms: item.updated_at.as_millis(),
                counter: 0,
                device,
            };
            self.device
                .clock
                .observe(stamp, Timestamp::now().as_millis());
            pending.push(Pending {
                state: ItemState::live(SyncItem::capture(&item, stamp)),
                version: item.version,
            });
        }
        self.commit_states(pending)
    }

    pub(crate) fn write_device(&self, device: &DeviceRecord) -> Result<()> {
        self.write_record(&DEVICE_RECORD_ID, &RecordBody::Device(device.clone()))
    }

    fn write_sync(&self, sync: &ItemSyncRecord) -> Result<()> {
        self.write_record(
            &item_sync_record_id(sync.item_id),
            &RecordBody::ItemSync(sync.clone()),
        )
    }
}

#[allow(clippy::large_enum_variant)] // short-lived, built in a small batch
enum Prepared {
    Live(ItemEntry),
    Purged(ItemSyncRecord),
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview, VaultId};

    use super::*;
    use crate::store::test_support::{credentials, new_session};
    use crate::{Session, Store};

    /// Two devices sharing the same vaults: B is A restored from a backup.
    fn pair() -> (tempfile::TempDir, Session, Session) {
        let (dir, a) = new_session();
        let backup = dir.path().join("a.qlbackup");
        a.backup_to(&backup).unwrap();
        let b = Store::restore_from(
            &backup,
            &dir.path().join("b.qlvault"),
            &credentials("master"),
        )
        .unwrap();
        (dir, a, b)
    }

    fn personal(s: &Session) -> VaultId {
        s.vault_by_name("Personal").unwrap().id
    }

    fn add(s: &mut Session, title: &str) -> ItemId {
        let v = personal(s);
        s.add_item(
            v,
            ItemKind::Login,
            ItemOverview::new(title).unwrap(),
            ItemDetails::with_password("p1".into()),
        )
        .unwrap()
    }

    fn sync(from: &Session, to: &mut Session) -> ApplyReport {
        to.apply_changes(&from.changes_since(None).unwrap())
            .unwrap()
    }

    fn titles(s: &Session) -> Vec<String> {
        s.items().iter().map(|i| i.overview.title.clone()).collect()
    }

    #[test]
    fn restored_backups_get_their_own_device_identity() {
        let (dir, mut a, b) = pair();
        assert_ne!(a.device_id(), b.device_id());
        assert_ne!(a.static_key().public(), b.static_key().public());
        assert_eq!(a.device_name(), b.device_name());
        a.add_peer(crate::Peer::new(
            DeviceId::new(),
            "x",
            [1; 32],
            Timestamp::now(),
        ))
        .unwrap();
        let backup = dir.path().join("again.qlbackup");
        a.backup_to(&backup).unwrap();
        let c = Store::restore_from(
            &backup,
            &dir.path().join("c.qlvault"),
            &credentials("master"),
        )
        .unwrap();
        assert!(c.peers().is_empty(), "peers belong to the original device");
    }

    #[test]
    fn new_items_reach_the_peer_once() {
        let (_d, mut a, mut b) = pair();
        let id = add(&mut a, "Mail");
        let report = sync(&a, &mut b);
        assert_eq!(report.changed, vec![id]);
        assert_eq!(titles(&b), vec!["Mail"]);
        assert_eq!(b.item(id).unwrap().details.password, Some("p1".into()));
        assert_eq!(
            sync(&a, &mut b),
            ApplyReport::default(),
            "nothing new the second time"
        );
    }

    #[test]
    fn changes_since_only_returns_newer_changes() {
        let (_d, mut a, _b) = pair();
        add(&mut a, "One");
        let seen = a.sync_seq();
        add(&mut a, "Two");
        let newer = a.changes_since(Some(seen)).unwrap();
        assert_eq!(newer.len(), 1);
        assert_eq!(a.changes_since(Some(a.sync_seq())).unwrap(), vec![]);
        assert_eq!(a.changes_since(None).unwrap().len(), 2);
    }

    #[test]
    fn edits_to_different_fields_on_two_devices_both_survive() {
        let (_d, mut a, mut b) = pair();
        let id = add(&mut a, "Mail");
        sync(&a, &mut b);
        let mut on_a = a.item(id).unwrap();
        on_a.set_overview(
            on_a.overview.clone().with_username("me@a"),
            Timestamp::now(),
        );
        a.save_item(&on_a).unwrap();
        let mut on_b = b.item(id).unwrap();
        let notes = ItemDetails::default().notes("from b".into()).notes;
        on_b.details.notes = notes.clone();
        on_b.touch(Timestamp::now());
        b.save_item(&on_b).unwrap();
        sync(&a, &mut b);
        sync(&b, &mut a);
        for s in [&a, &b] {
            let item = s.item(id).unwrap();
            assert_eq!(item.overview.username.as_deref(), Some("me@a"));
            assert_eq!(item.details.notes, notes);
        }
    }

    #[test]
    fn concurrent_password_changes_are_reported_and_kept() {
        let (_d, mut a, mut b) = pair();
        let id = add(&mut a, "Mail");
        sync(&a, &mut b);
        for (s, pw) in [(&mut a, "from-a"), (&mut b, "from-b")] {
            let mut item = s.item(id).unwrap();
            item.set_password(Some(pw.into()), Timestamp::now());
            s.save_item(&item).unwrap();
        }
        let report = sync(&a, &mut b);
        assert_eq!(report.conflicts, vec![id]);
        assert_eq!(
            b.items_to_review(),
            vec![id],
            "flagged until the user looks at it"
        );
        sync(&b, &mut a);
        let (on_a, on_b) = (a.item(id).unwrap(), b.item(id).unwrap());
        assert_eq!(on_a.details.password, on_b.details.password);
        for pw in ["from-a", "from-b", "p1"] {
            let pw = pw.into();
            assert!(
                on_a.details.password.as_ref() == Some(&pw)
                    || password365_sync::history::contains(&on_a.details.password_history, &pw)
            );
        }
    }

    #[test]
    fn deletions_and_restores_propagate() {
        let (_d, mut a, mut b) = pair();
        let id = add(&mut a, "Mail");
        sync(&a, &mut b);
        a.delete_item(id).unwrap();
        sync(&a, &mut b);
        assert!(b.items().is_empty());
        assert_eq!(b.deleted_items().len(), 1);
        b.restore_item(id).unwrap();
        sync(&b, &mut a);
        assert_eq!(titles(&a), vec!["Mail"]);
        assert!(a.deleted_items().is_empty());
    }

    #[test]
    fn purges_propagate_but_an_unseen_edit_wins() {
        let (_d, mut a, mut b) = pair();
        let gone = add(&mut a, "Gone");
        let kept = add(&mut a, "Kept");
        sync(&a, &mut b);
        let mut edited = b.item(kept).unwrap();
        edited.set_overview(ItemOverview::new("Edited").unwrap(), Timestamp::now());
        b.save_item(&edited).unwrap();
        a.delete_item(gone).unwrap();
        a.delete_item(kept).unwrap();
        let far_future = Timestamp::from_millis(i64::MAX);
        assert_eq!(a.purge_deleted(far_future).unwrap(), 2);
        assert!(a.deleted_items().is_empty());
        sync(&a, &mut b);
        assert_eq!(titles(&b), vec!["Edited"]);
        sync(&b, &mut a);
        assert_eq!(titles(&a), vec!["Edited"]);
    }

    #[test]
    fn purge_keeps_recent_deletions() {
        let (_d, mut a, _b) = pair();
        let id = add(&mut a, "Mail");
        a.delete_item(id).unwrap();
        assert_eq!(a.purge_deleted(Timestamp::from_millis(0)).unwrap(), 0);
        assert_eq!(a.deleted_items().len(), 1);
    }

    #[test]
    fn items_in_vaults_this_device_lacks_are_skipped() {
        let (_d, mut a, mut b) = pair();
        let work = a.create_vault("Work").unwrap();
        a.add_item(
            work,
            ItemKind::Login,
            ItemOverview::new("Job").unwrap(),
            ItemDetails::default(),
        )
        .unwrap();
        let report = sync(&a, &mut b);
        assert_eq!(report.skipped, 1);
        assert!(report.changed.is_empty());
    }

    #[test]
    fn sync_state_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.qlvault");
        let mut s = Store::create(
            &path,
            &credentials("pw"),
            password365_crypto::kdf::KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let id = add(&mut s, "Mail");
        s.delete_item(id).unwrap();
        let (device, seq) = (s.device_id(), s.sync_seq());
        s.lock();
        let s = Store::open(&path)
            .unwrap()
            .unlock(&credentials("pw"))
            .unwrap();
        assert_eq!((s.device_id(), s.sync_seq()), (device, seq));
        assert_eq!(s.deleted_items().len(), 1);
    }

    #[test]
    fn purge_markers_survive_reopening_and_are_sent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.qlvault");
        let mut s = Store::create(
            &path,
            &credentials("pw"),
            password365_crypto::kdf::KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let id = add(&mut s, "Mail");
        s.delete_item(id).unwrap();
        s.purge_deleted(Timestamp::from_millis(i64::MAX)).unwrap();
        s.lock();
        let s = Store::open(&path)
            .unwrap()
            .unlock(&credentials("pw"))
            .unwrap();
        let changes = s.changes_since(None).unwrap();
        assert!(matches!(changes.as_slice(), [ItemState::Purged { id: i, .. }] if *i == id));
        assert!(
            s.db.records()
                .unwrap()
                .iter()
                .all(|(rid, _)| rid != id.as_bytes()),
            "secrets wiped"
        );
    }

    #[test]
    fn items_from_before_sync_are_adopted_on_unlock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.qlvault");
        let mut s = Store::create(
            &path,
            &credentials("pw"),
            password365_crypto::kdf::KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let id = add(&mut s, "Old");
        // Simulate a store written before sync existed.
        s.db.delete_record(&item_sync_record_id(id)).unwrap();
        s.db.delete_record(&DEVICE_RECORD_ID).unwrap();
        s.lock();
        let s = Store::open(&path)
            .unwrap()
            .unlock(&credentials("pw"))
            .unwrap();
        assert_eq!(titles(&s), vec!["Old"]);
        assert_eq!(s.sync_seq(), 1);
        let changes = s.changes_since(None).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].latest().device == s.device_id());
    }

    #[test]
    fn deleting_a_vault_purges_its_deleted_items() {
        let (_d, mut a, _b) = pair();
        let work = a.create_vault("Work").unwrap();
        let id = a
            .add_item(
                work,
                ItemKind::Login,
                ItemOverview::new("Job").unwrap(),
                ItemDetails::default(),
            )
            .unwrap();
        a.delete_item(id).unwrap();
        a.delete_vault(work).unwrap();
        assert!(a.deleted_items().is_empty());
        assert!(matches!(
            a.state_of(id).unwrap(),
            Some(ItemState::Purged { .. })
        ));
    }

    #[test]
    fn recently_deleted_lists_newest_first_and_can_be_emptied_item_by_item() {
        let (_d, mut a, mut b) = pair();
        let first = add(&mut a, "First");
        let second = add(&mut a, "Second");
        a.delete_item(first).unwrap();
        a.delete_item(second).unwrap();
        let listed: Vec<ItemId> = a.deleted_items().iter().map(|(s, _)| s.id).collect();
        assert_eq!(listed, vec![second, first]);
        let (_, deleted_at) = a.deleted_items()[0];
        assert!(deleted_at.as_millis() > 0);
        a.purge_item(second).unwrap();
        assert!(matches!(a.purge_item(second), Err(VaultError::NotFound(_))));
        assert!(matches!(
            a.purge_item(ItemId::new()),
            Err(VaultError::NotFound(_))
        ));
        assert_eq!(a.deleted_items().len(), 1);
        sync(&a, &mut b);
        assert_eq!(
            b.deleted_items().len(),
            1,
            "the purged one never arrives as an item"
        );
    }

    #[test]
    fn deleted_items_cannot_be_edited_or_deleted_again() {
        let (_d, mut a, _b) = pair();
        let id = add(&mut a, "Mail");
        let mut item = a.item(id).unwrap();
        a.delete_item(id).unwrap();
        item.touch(Timestamp::now());
        assert!(matches!(a.save_item(&item), Err(VaultError::NotFound(_))));
        assert!(matches!(a.delete_item(id), Err(VaultError::NotFound(_))));
        assert!(matches!(a.item(id), Err(VaultError::NotFound(_))));
        assert!(matches!(
            a.restore_item(ItemId::new()),
            Err(VaultError::NotFound(_))
        ));
    }
}
