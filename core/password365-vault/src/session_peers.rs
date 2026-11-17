//! This device's pairing identity and the devices it is paired with.

use std::collections::BTreeMap;

use password365_model::{Timestamp, VaultId};
use password365_sync::DeviceId;
use password365_sync::pairing::StaticKey;
use serde::{Deserialize, Serialize};

use crate::db::RecordId;
use crate::records::{DeviceKeysRecord, ItemSyncBody, PeerRecord, RecordBody};
use crate::session::Session;
use crate::{Result, VaultError};

/// Name a new install shows until the app sets a better one.
pub const DEFAULT_DEVICE_NAME: &str = "This device";
/// Longest device name kept.
pub const MAX_DEVICE_NAME: usize = 64;

/// Record id of the device keys: fixed, version nibble 0xf (never a `UUIDv4`).
pub(crate) const DEVICE_KEYS_RECORD_ID: RecordId = {
    let mut id = [0xff; 16];
    id[15] = 0xfe;
    id
};

/// A paired device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    /// Its sync identity.
    pub device_id: DeviceId,
    /// Name it gave itself (e.g. "Laptop").
    pub name: String,
    /// Its long-term Noise key, pinned at pairing.
    pub static_public: [u8; 32],
    /// When it was paired.
    pub paired_at: Timestamp,
    /// Highest of *our* change sequences it confirmed having.
    pub acked_seq: Option<u64>,
    /// Highest of *its* change sequences we merged.
    pub received_seq: Option<u64>,
    /// Last successful sync.
    pub last_sync: Option<Timestamp>,
    /// Where it was last reachable on the local network (`host:port`).
    pub addr: Option<String>,
    /// The vaults it was given when it joined: the only ones it receives and
    /// may change. `None`: every vault (devices paired before this was kept,
    /// and the device that invited this one). Stored beside the peer record.
    #[serde(skip)]
    pub vaults: Option<Vec<VaultId>>,
}

impl Peer {
    /// A newly paired device that has not synced yet.
    #[must_use]
    pub fn new(device_id: DeviceId, name: &str, static_public: [u8; 32], now: Timestamp) -> Self {
        Self {
            device_id,
            name: clean_name(name),
            static_public,
            paired_at: now,
            acked_seq: None,
            received_seq: None,
            last_sync: None,
            addr: None,
            vaults: None,
        }
    }

    /// Whether this device may receive and change items of `vault`.
    #[must_use]
    pub fn may_use(&self, vault: VaultId) -> bool {
        self.vaults.as_ref().is_none_or(|v| v.contains(&vault))
    }
}

impl DeviceKeysRecord {
    /// A fresh pairing key with the default name.
    pub(crate) fn generate() -> Result<Self> {
        let key = StaticKey::generate().map_err(|_| VaultError::Corrupt("pairing key"))?;
        Ok(Self {
            static_private: *key.private(),
            static_public: key.public(),
            name: DEFAULT_DEVICE_NAME.to_owned(),
        })
    }
}

impl Session {
    /// The name other devices see for this one.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_keys.name
    }

    /// Renames this device (trimmed, capped; blank keeps the current name).
    pub fn set_device_name(&mut self, name: &str) -> Result<()> {
        let name = clean_name(name);
        if name.is_empty() {
            return Ok(());
        }
        let mut keys = self.device_keys.clone();
        keys.name = name;
        self.write_record(
            &DEVICE_KEYS_RECORD_ID,
            &RecordBody::DeviceKeys(keys.clone()),
        )?;
        self.device_keys = keys;
        Ok(())
    }

    /// This device's long-term pairing key.
    #[must_use]
    pub fn static_key(&self) -> StaticKey {
        StaticKey::from_parts(
            self.device_keys.static_private,
            self.device_keys.static_public,
        )
    }

    /// Paired devices, by name.
    #[must_use]
    pub fn peers(&self) -> Vec<&Peer> {
        let mut list: Vec<&Peer> = self.peers.values().collect();
        list.sort_by_key(|p| p.name.to_lowercase());
        list
    }

    /// One paired device.
    pub fn peer(&self, id: DeviceId) -> Result<&Peer> {
        self.peers.get(&id).ok_or(VaultError::NotFound("device"))
    }

    /// Adds (or replaces) a paired device.
    pub fn add_peer(&mut self, peer: Peer) -> Result<()> {
        if self.is_revoked(peer.device_id) {
            return Err(VaultError::RevokedDevice);
        }
        self.write_peer(&peer)?;
        self.peers.insert(peer.device_id, peer);
        Ok(())
    }

    /// Pins a device that has just paired. Unlike [`Self::add_peer`] it never
    /// replaces anything: a new device cannot take over the id or the key of a
    /// device already paired, nor this device's own.
    pub fn pair_peer(&mut self, peer: Peer) -> Result<()> {
        let taken = peer.device_id == self.device_id()
            || peer.static_public == self.device_keys.static_public
            || self
                .peers
                .values()
                .any(|p| p.device_id == peer.device_id || p.static_public == peer.static_public);
        if taken {
            return Err(VaultError::AlreadyPaired);
        }
        self.add_peer(peer)
    }

    /// Forgets a paired device (it can no longer sync with this one).
    pub fn remove_peer(&mut self, id: DeviceId) -> Result<()> {
        self.peer(id)?;
        self.db.delete_record(&peer_record_id(id))?;
        self.peers.remove(&id);
        Ok(())
    }

    /// Records sync progress: what the peer confirmed having and what we merged.
    /// Both only move forward.
    pub fn record_sync(
        &mut self,
        id: DeviceId,
        acked_seq: Option<u64>,
        received_seq: Option<u64>,
        at: Timestamp,
    ) -> Result<()> {
        let mut peer = self.peer(id)?.clone();
        peer.acked_seq = max_option(peer.acked_seq, acked_seq);
        peer.received_seq = max_option(peer.received_seq, received_seq);
        peer.last_sync = Some(at);
        self.add_peer(peer)
    }

    /// Remembers where a paired device can be reached.
    pub fn set_peer_addr(&mut self, id: DeviceId, addr: Option<String>) -> Result<()> {
        let mut peer = self.peer(id)?.clone();
        peer.addr = addr;
        self.add_peer(peer)
    }

    /// How many local changes `id` has not confirmed yet.
    pub fn changes_pending_for(&self, id: DeviceId) -> Result<usize> {
        let acked = self.peer(id)?.acked_seq;
        let newer = |seq: u64| acked.is_none_or(|a| seq > a);
        let items = self.items.values().filter(|e| newer(e.sync.seq)).count();
        let purged = self
            .purged
            .values()
            .filter(|s| matches!(s.body, ItemSyncBody::Purged(_)) && newer(s.seq))
            .count();
        Ok(items + purged)
    }

    pub(crate) fn load_peers(&mut self, peers: Vec<PeerRecord>) {
        self.peers = peers
            .into_iter()
            .map(|r| {
                let mut peer = r.peer;
                peer.vaults = r.vaults;
                (peer.device_id, peer)
            })
            .collect::<BTreeMap<_, _>>();
    }

    fn write_peer(&self, peer: &Peer) -> Result<()> {
        self.write_record(
            &peer_record_id(peer.device_id),
            &RecordBody::PeerWithVaults(PeerRecord {
                peer: peer.clone(),
                vaults: peer.vaults.clone(),
            }),
        )
    }
}

/// Record id of a peer: its device id with version nibble 0xe.
fn peer_record_id(id: DeviceId) -> RecordId {
    let mut out = *id.as_bytes();
    out[6] = (out[6] & 0x0f) | 0xe0;
    out
}

fn max_option(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

fn clean_name(name: &str) -> String {
    name.trim().chars().take(MAX_DEVICE_NAME).collect()
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview};

    use super::*;
    use crate::Store;
    use crate::store::test_support::{credentials, new_session};

    fn peer(name: &str) -> Peer {
        Peer::new(DeviceId::new(), name, [3; 32], Timestamp::from_millis(5))
    }

    #[test]
    fn a_new_device_cannot_take_over_a_paired_one() {
        let (_d, mut s) = new_session();
        let phone = Peer::new(DeviceId::new(), "Phone", [4; 32], Timestamp::from_millis(5));
        s.pair_peer(phone.clone()).unwrap();
        let same_id = Peer::new(phone.device_id, "Evil", [9; 32], Timestamp::from_millis(6));
        let same_key = Peer::new(DeviceId::new(), "Evil", [4; 32], Timestamp::from_millis(6));
        let as_me = Peer::new(s.device_id(), "Evil", [8; 32], Timestamp::from_millis(6));
        for evil in [same_id, same_key, as_me] {
            assert!(matches!(s.pair_peer(evil), Err(VaultError::AlreadyPaired)));
        }
        assert_eq!(s.peer(phone.device_id).unwrap().name, "Phone");
    }

    #[test]
    fn a_new_store_has_a_pairing_key_and_default_name() {
        let (_d, s) = new_session();
        assert_eq!(s.device_name(), DEFAULT_DEVICE_NAME);
        assert_ne!(s.static_key().public(), [0; 32]);
    }

    #[test]
    fn device_name_is_cleaned_and_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.qlvault");
        let mut s = Store::create(
            &path,
            &credentials("pw"),
            password365_crypto::kdf::KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let key = s.static_key().public();
        s.set_device_name("  MacBook Air  ").unwrap();
        s.set_device_name("   ").unwrap();
        assert_eq!(s.device_name(), "MacBook Air");
        s.set_device_name(&"x".repeat(100)).unwrap();
        assert_eq!(s.device_name().len(), MAX_DEVICE_NAME);
        s.set_device_name("Desk").unwrap();
        s.lock();
        let s = Store::open(&path)
            .unwrap()
            .unlock(&credentials("pw"))
            .unwrap();
        assert_eq!(s.device_name(), "Desk");
        assert_eq!(s.static_key().public(), key);
    }

    #[test]
    fn peers_are_added_listed_and_removed() {
        let (_d, mut s) = new_session();
        let (b, a) = (peer("phone"), peer(" Air "));
        s.add_peer(b.clone()).unwrap();
        s.add_peer(a.clone()).unwrap();
        let names: Vec<&str> = s.peers().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["Air", "phone"]);
        s.set_peer_addr(b.device_id, Some("10.0.0.5:7480".into()))
            .unwrap();
        assert_eq!(
            s.peer(b.device_id).unwrap().addr.as_deref(),
            Some("10.0.0.5:7480")
        );
        s.remove_peer(b.device_id).unwrap();
        assert!(s.set_peer_addr(b.device_id, None).is_err());
        assert!(matches!(
            s.remove_peer(b.device_id),
            Err(VaultError::NotFound(_))
        ));
        assert!(matches!(s.peer(b.device_id), Err(VaultError::NotFound(_))));
        assert_eq!(s.peers().len(), 1);
    }

    #[test]
    fn sync_progress_only_moves_forward_and_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.qlvault");
        let mut s = Store::create(
            &path,
            &credentials("pw"),
            password365_crypto::kdf::KdfParams::insecure_for_tests(),
        )
        .unwrap();
        let p = peer("phone");
        s.add_peer(p.clone()).unwrap();
        s.record_sync(p.device_id, Some(5), None, Timestamp::from_millis(10))
            .unwrap();
        s.record_sync(p.device_id, Some(3), Some(7), Timestamp::from_millis(20))
            .unwrap();
        s.record_sync(p.device_id, Some(4), Some(2), Timestamp::from_millis(30))
            .unwrap();
        s.lock();
        let s = Store::open(&path)
            .unwrap()
            .unlock(&credentials("pw"))
            .unwrap();
        let stored = s.peer(p.device_id).unwrap();
        assert_eq!((stored.acked_seq, stored.received_seq), (Some(5), Some(7)));
        assert_eq!(stored.last_sync, Some(Timestamp::from_millis(30)));
    }

    #[test]
    fn pending_changes_count_what_the_peer_has_not_confirmed() {
        let (_d, mut s) = new_session();
        let p = peer("phone");
        s.add_peer(p.clone()).unwrap();
        let vault = s.vault_by_name("Personal").unwrap().id;
        let add = |s: &mut Session, t: &str| {
            s.add_item(
                vault,
                ItemKind::Login,
                ItemOverview::new(t).unwrap(),
                ItemDetails::default(),
            )
            .unwrap()
        };
        let first = add(&mut s, "One");
        add(&mut s, "Two");
        assert_eq!(s.changes_pending_for(p.device_id).unwrap(), 2);
        s.record_sync(p.device_id, Some(s.sync_seq()), None, Timestamp::now())
            .unwrap();
        assert_eq!(s.changes_pending_for(p.device_id).unwrap(), 0);
        s.delete_item(first).unwrap();
        s.purge_deleted(Timestamp::from_millis(i64::MAX)).unwrap();
        assert_eq!(
            s.changes_pending_for(p.device_id).unwrap(),
            1,
            "the purge marker"
        );
        assert!(s.changes_pending_for(DeviceId::new()).is_err());
    }

    #[test]
    fn peer_record_ids_never_collide_with_uuid_v4_ids() {
        let id = DeviceId::new();
        let rid = peer_record_id(id);
        assert_eq!(rid[6] >> 4, 0xe);
        assert_eq!(DEVICE_KEYS_RECORD_ID[6] >> 4, 0xf);
        assert_ne!(DEVICE_KEYS_RECORD_ID, crate::sync_meta::DEVICE_RECORD_ID);
    }
}
