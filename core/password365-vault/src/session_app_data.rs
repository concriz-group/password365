//! Opaque data the app keeps sealed in this store (this device only, never
//! synced): the security settings, so a program that edits the settings file
//! cannot loosen them unnoticed.

use crate::Result;
use crate::db::RecordId;
use crate::records::RecordBody;
use crate::session::Session;

/// Record id of the app data: fixed, version nibble 0xf (never a `UUIDv4`).
pub(crate) const APP_DATA_RECORD_ID: RecordId = {
    let mut id = [0xff; 16];
    id[15] = 0xfa;
    id
};

impl Session {
    /// The app's sealed data, if it stored any.
    #[must_use]
    pub fn app_data(&self) -> Option<&[u8]> {
        self.app_data.as_deref()
    }

    /// Replaces the app's sealed data.
    pub fn set_app_data(&mut self, data: &[u8]) -> Result<()> {
        if self.app_data.as_deref() == Some(data) {
            return Ok(());
        }
        self.write_record(&APP_DATA_RECORD_ID, &RecordBody::AppData(data.to_vec()))?;
        self.app_data = Some(data.to_vec());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Store;
    use crate::store::test_support::{credentials, new_session};

    #[test]
    fn app_data_is_kept_sealed_across_unlocks() {
        let (dir, mut s) = new_session();
        assert!(s.app_data().is_none());
        s.set_app_data(b"{\"auto_lock_minutes\":5}").unwrap();
        s.lock();
        let s = Store::open(&dir.path().join("test.qlvault"))
            .unwrap()
            .unlock(&credentials("master"))
            .unwrap();
        assert_eq!(s.app_data(), Some(&b"{\"auto_lock_minutes\":5}"[..]));
    }
}
