//! Quick unlock (Face ID / Touch ID): see `password365_vault::quick_unlock`.
//! The app keeps the wrapped copy next to the store; the platform keeps the
//! device key behind biometry. After 14 days, a master password change, or a
//! key that no longer fits, the copy is deleted and the master password is
//! asked for (then quick unlock can be set up again).

use std::io::Write;
use std::path::PathBuf;

use password365_crypto::key::SymmetricKey;
use password365_model::Timestamp;
use password365_vault::{Store, VaultError};
use zeroize::Zeroizing;

use crate::{AppError, AppService, Result};

/// How long a quick-unlock copy lasts before the master password is needed again.
pub const QUICK_UNLOCK_DAYS: i64 = 14;
const MAX_AGE_MS: i64 = QUICK_UNLOCK_DAYS * 24 * 60 * 60 * 1000;

impl AppService {
    fn quick_path(&self) -> PathBuf {
        let mut path = self.config.store_path.clone().into_os_string();
        path.push(".quick");
        PathBuf::from(path)
    }

    /// Whether a quick-unlock copy is set up on this device.
    #[must_use]
    pub fn quick_unlock_ready(&self) -> bool {
        self.quick_path().exists()
    }

    /// Sets up quick unlock (vault unlocked): returns the 32-byte device key
    /// the platform must keep behind biometry. Replaces any earlier copy.
    pub fn enable_quick_unlock(&mut self) -> Result<Zeroizing<Vec<u8>>> {
        let device_key = SymmetricKey::generate()?;
        let sealed = self
            .session()?
            .seal_quick_unlock(&device_key, Timestamp::now())?;
        let path = self.quick_path();
        let _ = std::fs::remove_file(&path);
        password365_vault::private_file::create_private_file(&path)
            .and_then(|mut f| f.write_all(&sealed))
            .map_err(|e| AppError::Io(format!("cannot save quick unlock: {e}")))?;
        Ok(Zeroizing::new(device_key.expose().to_vec()))
    }

    /// Unlocks with the device key the platform released after biometry.
    /// `PasswordRequired`: the master password is needed (the copy is gone).
    pub fn unlock_quick(&mut self, device_key: &[u8]) -> Result<()> {
        let key: [u8; 32] = device_key
            .try_into()
            .map_err(|_| AppError::PasswordRequired)?;
        let sealed = Zeroizing::new(
            std::fs::read(self.quick_path()).map_err(|_| AppError::PasswordRequired)?,
        );
        let opened = Store::open(&self.config.store_path)?.unlock_quick(
            &SymmetricKey::from_bytes(key),
            &sealed,
            Timestamp::now(),
            MAX_AGE_MS,
        );
        match opened {
            Ok(session) => {
                self.start_session(session);
                self.purge_expired_deletions(Timestamp::now())
            }
            Err(VaultError::QuickUnlockExpired | VaultError::Corrupt(_)) => {
                self.disable_quick_unlock();
                Err(AppError::PasswordRequired)
            }
            Err(other) => Err(other.into()),
        }
    }

    /// Forgets the quick-unlock copy (the platform deletes its key too).
    pub fn disable_quick_unlock(&mut self) {
        let _ = std::fs::remove_file(self.quick_path());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::tests::{PW, with_account};

    #[test]
    fn unlocks_with_the_device_key_and_falls_back_to_the_password() {
        let mut fx = with_account();
        assert!(!fx.app.quick_unlock_ready());
        let key = fx.app.enable_quick_unlock().unwrap();
        assert!(fx.app.quick_unlock_ready());
        fx.app.lock();
        fx.app.unlock_quick(&key).unwrap();
        assert!(fx.app.status().unlocked);

        fx.app.lock();
        assert_eq!(
            fx.app.unlock_quick(&[7; 32]).unwrap_err(),
            AppError::PasswordRequired,
            "another key"
        );
        assert!(!fx.app.quick_unlock_ready(), "a copy that failed is gone");
        assert_eq!(
            fx.app.unlock_quick(&key).unwrap_err(),
            AppError::PasswordRequired
        );
        assert_eq!(
            fx.app.unlock_quick(&[1, 2]).unwrap_err(),
            AppError::PasswordRequired
        );

        fx.app.unlock(PW.into(), None).unwrap();
        fx.app.enable_quick_unlock().unwrap();
        fx.app.disable_quick_unlock();
        assert!(!fx.app.quick_unlock_ready());
        fx.app.lock();
        assert!(
            fx.app.enable_quick_unlock().is_err(),
            "needs an unlocked vault"
        );
    }
}
