//! Face ID / Touch ID unlock and the lock and clipboard settings.

use crate::{Password365, Result};

/// The settings the phone shows.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MobileSettings {
    /// Lock after the app has been away this many minutes (0: at once).
    pub auto_lock_minutes: u32,
    /// Copied secrets expire after this many seconds (0: never).
    pub clipboard_clear_seconds: u32,
    /// Country whose fields new items follow ("" from the system).
    pub region: String,
}

/// A phone locks sooner than a computer (PLAN §3.4: 1 minute on mobile).
pub(crate) const MOBILE_AUTO_LOCK_MINUTES: u32 = 1;

#[uniffi::export]
impl Password365 {
    /// Whether Face ID / Touch ID unlock is set up here.
    pub fn quick_unlock_ready(&self) -> bool {
        self.app().quick_unlock_ready()
    }

    /// Sets up Face ID / Touch ID unlock (vault unlocked); returns the key
    /// the app must keep in the Keychain behind biometry.
    pub fn enable_quick_unlock(&self) -> Result<Vec<u8>> {
        Ok(self.app().enable_quick_unlock()?.to_vec())
    }

    /// Unlocks with the key the Keychain released after Face ID / Touch ID.
    /// Fails with `password_required` when the master password is needed.
    pub fn unlock_quick(&self, key: Vec<u8>) -> Result<()> {
        let key = zeroize::Zeroizing::new(key);
        Ok(self.app().unlock_quick(&key)?)
    }

    /// Turns Face ID / Touch ID unlock off.
    pub fn disable_quick_unlock(&self) {
        self.app().disable_quick_unlock();
    }

    /// The phone's settings.
    pub fn mobile_settings(&self) -> MobileSettings {
        let s = self.app().settings();
        MobileSettings {
            auto_lock_minutes: s.auto_lock_minutes,
            clipboard_clear_seconds: s.clipboard_clear_seconds,
            region: s.region,
        }
    }

    /// Saves the phone's settings.
    pub fn set_mobile_settings(&self, settings: MobileSettings) -> Result<()> {
        let mut app = self.app();
        let mut all = app.settings();
        all.auto_lock_minutes = settings.auto_lock_minutes;
        all.clipboard_clear_seconds = settings.clipboard_clear_seconds;
        all.region = settings.region;
        Ok(app.update_settings(all)?)
    }
}
