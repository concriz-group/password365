//! Master password changes across devices.
//!
//! Each store wraps its own storage key with the account credentials, so a
//! change made on one device has to be applied by every other one. The
//! device that changes the password records a [`PasswordChange`]: a stamp
//! and a *verifier* (a header wrapping a throwaway key with the new
//! credentials). Syncs carry the newest change. A device that receives a
//! newer one marks it pending; when the user types the new password there,
//! it is checked against the verifier and the local store is re-wrapped.
//! Neither the password nor any key travels.

use password365_crypto::kdf::KdfParams;
use password365_model::Timestamp;
use password365_sync::{DeviceId, Hlc};
use serde::{Deserialize, Serialize};

use crate::db::RecordId;
use crate::header::Header;
use crate::records::{PasswordRecord, RecordBody};
use crate::session::Session;
use crate::{Credentials, Result, VaultError};

/// Fixed record id (version nibble 0xf: never a `UUIDv4`).
pub(crate) const PASSWORD_RECORD_ID: RecordId = {
    let mut id = [0xff; 16];
    id[15] = 0xfb;
    id
};

/// The latest master password change of the account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordChange {
    stamp: Hlc,
    verifier: Header,
}

impl PasswordChange {
    /// The device where the password was changed.
    #[must_use]
    pub const fn changed_by(&self) -> DeviceId {
        self.stamp.device
    }

    /// When it was changed.
    #[must_use]
    pub const fn changed_at(&self) -> Timestamp {
        Timestamp::from_millis(self.stamp.wall_ms)
    }
}

impl Session {
    /// Changes the master password here and records the change for the other
    /// devices.
    pub fn change_master_password(&mut self, new: &Credentials, kdf: KdfParams) -> Result<()> {
        let (verifier, _) = Header::create(new, kdf)?;
        self.change_credentials(new, kdf)?;
        let stamp = self.next_stamp();
        self.write_device(&self.device)?;
        self.save_password(PasswordRecord {
            latest: PasswordChange { stamp, verifier },
            applied: true,
        })
    }

    /// The account's latest password change known here.
    #[must_use]
    pub fn password_change(&self) -> Option<&PasswordChange> {
        self.password.as_ref().map(|r| &r.latest)
    }

    /// A change made on another device that this store does not use yet.
    #[must_use]
    pub fn pending_password_change(&self) -> Option<&PasswordChange> {
        self.password
            .as_ref()
            .filter(|r| !r.applied)
            .map(|r| &r.latest)
    }

    /// Keeps `incoming` if it is newer than what is known here; it is then
    /// pending until [`Self::adopt_password_change`]. Returns `true` if so.
    pub fn merge_password_change(&mut self, incoming: PasswordChange) -> Result<bool> {
        // Dated far ahead, it would beat every later change of password.
        let now = Timestamp::now().as_millis();
        if incoming.stamp.wall_ms > now.saturating_add(crate::session_sync::MAX_FUTURE_MS) {
            return Err(VaultError::ClockAhead);
        }
        if self
            .password_change()
            .is_some_and(|known| known.stamp >= incoming.stamp)
        {
            return Ok(false);
        }
        self.device
            .clock
            .observe(incoming.stamp, Timestamp::now().as_millis());
        self.write_device(&self.device)?;
        self.save_password(PasswordRecord {
            latest: incoming,
            applied: false,
        })?;
        Ok(true)
    }

    /// Uses the new master password here. Fails with `WrongCredentials` if
    /// `new` is not the password set on the other device.
    pub fn adopt_password_change(&mut self, new: &Credentials, kdf: KdfParams) -> Result<()> {
        let Some(record) = self.password.clone().filter(|r| !r.applied) else {
            return Ok(());
        };
        record.latest.verifier.unlock(new)?;
        self.change_credentials(new, kdf)?;
        self.save_password(PasswordRecord {
            applied: true,
            ..record
        })
    }

    /// A joining device already uses the inviter's current password.
    pub(crate) fn adopt_joined_password(&mut self, change: Option<PasswordChange>) -> Result<()> {
        match change {
            Some(latest) => self.save_password(PasswordRecord {
                latest,
                applied: true,
            }),
            None => Ok(()),
        }
    }

    pub(crate) fn load_password(&mut self, record: Option<PasswordRecord>) {
        self.password = record;
    }

    fn save_password(&mut self, record: PasswordRecord) -> Result<()> {
        self.write_record(&PASSWORD_RECORD_ID, &RecordBody::Password(record.clone()))?;
        self.password = Some(record);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use password365_crypto::kdf::KdfParams;

    use crate::store::test_support::{credentials, new_session};
    use crate::{Store, VaultError};

    fn fast() -> KdfParams {
        KdfParams::insecure_for_tests()
    }

    #[test]
    fn a_change_made_elsewhere_is_pending_until_the_new_password_is_typed() {
        let (_da, mut a) = new_session();
        let (db, mut b) = new_session();
        a.change_master_password(&credentials("new"), fast())
            .unwrap();
        assert!(a.pending_password_change().is_none(), "applied where made");
        let change = a.password_change().unwrap().clone();
        assert_eq!(change.changed_by(), a.device_id());

        assert!(b.merge_password_change(change.clone()).unwrap());
        assert!(!b.merge_password_change(change).unwrap(), "already known");
        assert!(b.pending_password_change().is_some());
        assert!(matches!(
            b.adopt_password_change(&credentials("old"), fast()),
            Err(VaultError::WrongCredentials)
        ));
        b.adopt_password_change(&credentials("new"), fast())
            .unwrap();
        assert!(b.pending_password_change().is_none());
        b.adopt_password_change(&credentials("new"), fast())
            .unwrap();

        drop(b);
        let path = db.path().join("test.qlvault");
        assert!(
            Store::open(&path)
                .unwrap()
                .unlock(&credentials("master"))
                .is_err()
        );
        let b = Store::open(&path)
            .unwrap()
            .unlock(&credentials("new"))
            .unwrap();
        assert!(b.pending_password_change().is_none(), "survives reopening");
    }

    #[test]
    fn a_change_dated_far_ahead_is_refused() {
        let (_da, mut a) = new_session();
        let (_db, mut b) = new_session();
        a.change_master_password(&credentials("new"), fast())
            .unwrap();
        let mut change = a.password_change().unwrap().clone();
        change.stamp.wall_ms += 2 * crate::session_sync::MAX_FUTURE_MS;
        assert!(matches!(
            b.merge_password_change(change),
            Err(VaultError::ClockAhead)
        ));
        assert!(b.pending_password_change().is_none());
    }

    #[test]
    fn an_older_change_never_replaces_a_newer_one() {
        let (_da, mut a) = new_session();
        let (_db, mut b) = new_session();
        a.change_master_password(&credentials("first"), fast())
            .unwrap();
        let older = a.password_change().unwrap().clone();
        b.merge_password_change(older.clone()).unwrap();
        b.adopt_password_change(&credentials("first"), fast())
            .unwrap();
        b.change_master_password(&credentials("second"), fast())
            .unwrap();
        assert!(!b.merge_password_change(older).unwrap());
        assert!(b.pending_password_change().is_none());
        assert!(
            a.merge_password_change(b.password_change().unwrap().clone())
                .unwrap()
        );
    }
}
