//! Removed (revoked) devices.
//!
//! Removing a device adds it to a grow-only list that every sync carries to
//! the other paired devices, so all of them stop syncing with it — not only
//! the one where it was removed. A revoked device is never paired again
//! under the same identity.
//!
//! What revocation cannot do (no server): take back what the removed device
//! already has (still encrypted with the master password). If it was lost or
//! stolen, change the passwords of the important logins themselves.

use std::collections::BTreeSet;

use password365_sync::DeviceId;

use crate::Result;
use crate::db::RecordId;
use crate::records::{RecordBody, RevokedRecord};
use crate::session::Session;

/// Fixed record id (version nibble 0xf: never a `UUIDv4`).
pub(crate) const REVOKED_RECORD_ID: RecordId = {
    let mut id = [0xff; 16];
    id[15] = 0xfc;
    id
};

impl Session {
    /// Devices removed from this account.
    #[must_use]
    pub fn revoked_devices(&self) -> Vec<DeviceId> {
        self.revoked.iter().copied().collect()
    }

    /// `true` when `id` was removed from this account.
    #[must_use]
    pub fn is_revoked(&self, id: DeviceId) -> bool {
        self.revoked.contains(&id)
    }

    /// Removes a device: forgets it here and tells every other device on the
    /// next sync.
    pub fn revoke_device(&mut self, id: DeviceId) -> Result<()> {
        self.merge_revoked(&[id]).map(|_| ())
    }

    /// Adds revocations learnt from a peer; forgets those devices if paired.
    /// This device's own id is ignored (others refuse it anyway). Returns
    /// the newly revoked ids.
    pub fn merge_revoked(&mut self, ids: &[DeviceId]) -> Result<Vec<DeviceId>> {
        let own = self.device_id();
        let fresh: Vec<DeviceId> = ids
            .iter()
            .copied()
            .filter(|id| *id != own && !self.revoked.contains(id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if fresh.is_empty() {
            return Ok(fresh);
        }
        let mut next = self.revoked.clone();
        next.extend(fresh.iter().copied());
        let record = RevokedRecord {
            devices: next.iter().copied().collect(),
        };
        self.write_record(&REVOKED_RECORD_ID, &RecordBody::Revoked(record))?;
        self.revoked = next;
        for id in &fresh {
            if self.peers.contains_key(id) {
                self.remove_peer(*id)?;
            }
        }
        Ok(fresh)
    }

    pub(crate) fn load_revoked(&mut self, record: Option<RevokedRecord>) {
        self.revoked = record
            .map(|r| r.devices.into_iter().collect())
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use password365_model::Timestamp;

    use crate::Peer;
    use crate::store::test_support::{credentials, new_session};
    use crate::{Store, VaultError};

    fn peer(name: &str) -> Peer {
        let id = password365_sync::DeviceId::new();
        Peer::new(id, name, [7; 32], Timestamp::now())
    }

    #[test]
    fn revoking_forgets_the_device_and_survives_reopening() {
        let (dir, mut s) = new_session();
        let phone = peer("Phone");
        s.add_peer(phone.clone()).unwrap();
        s.revoke_device(phone.device_id).unwrap();
        assert!(s.peers().is_empty());
        assert!(s.is_revoked(phone.device_id));
        drop(s);
        let s = Store::open(&dir.path().join("test.qlvault"))
            .unwrap()
            .unlock(&credentials("master"))
            .unwrap();
        assert_eq!(s.revoked_devices(), vec![phone.device_id]);
    }

    #[test]
    fn merging_reports_only_new_ids_and_never_this_device() {
        let (_d, mut s) = new_session();
        let (a, b) = (peer("A"), peer("B"));
        s.add_peer(b.clone()).unwrap();
        let own = s.device_id();
        let fresh = s
            .merge_revoked(&[a.device_id, b.device_id, a.device_id, own])
            .unwrap();
        assert_eq!(fresh.len(), 2);
        assert!(!s.is_revoked(own));
        assert!(s.peers().is_empty(), "a revoked peer is forgotten");
        assert!(s.merge_revoked(&[a.device_id]).unwrap().is_empty());
    }

    #[test]
    fn a_revoked_device_cannot_be_paired_again() {
        let (_d, mut s) = new_session();
        let phone = peer("Phone");
        s.revoke_device(phone.device_id).unwrap();
        assert!(matches!(
            s.add_peer(phone).unwrap_err(),
            VaultError::RevokedDevice
        ));
    }
}
