//! Background timer that enforces the idle auto-lock, locks when the computer
//! wakes from sleep or its screen gets locked, and notifies the UI.
//!
//! Sleep is noticed the same way on every system: this thread wakes every
//! [`CHECK_INTERVAL`]; while the computer sleeps it cannot, so on wake-up the
//! wall clock has jumped well past the interval.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use password365_model::Timestamp;

use crate::state::AppState;

/// How often the idle timer is checked.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(10);
/// A gap this much longer than [`CHECK_INTERVAL`] means the computer slept.
pub const SLEEP_GAP: Duration = Duration::from_secs(30);

/// Notices that the computer slept between two checks.
#[derive(Debug)]
pub struct SleepDetector {
    last: Timestamp,
}

impl SleepDetector {
    /// Starts watching at `now`.
    #[must_use]
    pub const fn new(now: Timestamp) -> Self {
        Self { last: now }
    }

    /// `true` if the computer slept since the previous check.
    pub fn woke(&mut self, now: Timestamp) -> bool {
        let gap = now.as_millis() - self.last.as_millis();
        self.last = now;
        let limit = CHECK_INTERVAL + SLEEP_GAP;
        gap > i64::try_from(limit.as_millis()).unwrap_or(i64::MAX)
    }
}

/// Locks after the computer woke from sleep or its screen was locked (unless
/// Settings › Security says not to); returns `true` if it was unlocked and is
/// now locked.
pub fn lock_after_sleep(state: &AppState) -> bool {
    state
        .with(|s| {
            if !s.settings().lock_on_sleep {
                return Ok(false);
            }
            let was_unlocked = s.status().unlocked;
            s.lock();
            Ok(was_unlocked)
        })
        .unwrap_or(false)
}

/// Checks the idle timeout once; returns `true` if the vault was locked now.
pub fn tick(state: &AppState, now: Timestamp) -> bool {
    state.with(|s| Ok(s.check_auto_lock(now))).unwrap_or(false)
}

/// Spawns the timer thread. `notify` is called whenever it locks the vault.
pub fn spawn(state: Arc<AppState>, notify: impl Fn() + Send + 'static) {
    thread::spawn(move || {
        let mut sleep = SleepDetector::new(Timestamp::now());
        let mut screen = crate::screen_lock::ScreenWatch::default();
        loop {
            thread::sleep(CHECK_INTERVAL);
            let now = Timestamp::now();
            let screen_locked = screen.just_locked(crate::screen_lock::is_locked());
            let slept = (sleep.woke(now) || screen_locked) && lock_after_sleep(&state);
            if slept || tick(&state, now) {
                notify();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use password365_app::AppConfig;
    use password365_app::secret_key_store::MemorySecretKeyStore;
    use password365_crypto::kdf::KdfParams;

    use super::*;

    #[test]
    fn a_jump_in_the_clock_means_the_computer_slept() {
        let mut d = SleepDetector::new(Timestamp::from_millis(0));
        assert!(!d.woke(Timestamp::from_millis(10_000)), "a normal tick");
        assert!(!d.woke(Timestamp::from_millis(45_000)), "a slow tick");
        assert!(d.woke(Timestamp::from_millis(200_000)), "slept");
        assert!(!d.woke(Timestamp::from_millis(210_000)));
    }

    #[test]
    fn waking_up_locks_an_unlocked_vault_once() {
        let (_dir, state) = unlocked();
        assert!(lock_after_sleep(&state));
        assert!(!state.with(|s| Ok(s.status())).unwrap().unlocked);
        assert!(!lock_after_sleep(&state), "already locked: nothing to tell");
    }

    #[test]
    fn sleep_can_be_left_unlocked_by_choice() {
        let (_dir, state) = unlocked();
        state
            .with(|s| {
                s.update_settings(password365_app::settings::Settings {
                    lock_on_sleep: false,
                    ..password365_app::settings::Settings::default()
                })
            })
            .unwrap();
        assert!(!lock_after_sleep(&state));
        assert!(state.with(|s| Ok(s.status())).unwrap().unlocked);
    }

    fn unlocked() -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig {
            store_path: dir.path().join("s"),
            kdf: KdfParams::insecure_for_tests(),
            auto_lock_ms: 0,
            settings_path: None,
        };
        let state = AppState::new(password365_app::AppService::new(
            config,
            Box::new(MemorySecretKeyStore::default()),
        ));
        let pw = "long enough password";
        state
            .with(|s| s.create_account(pw.into(), &pw.into()))
            .unwrap();
        (dir, state)
    }

    #[test]
    fn tick_locks_after_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig {
            store_path: dir.path().join("s"),
            kdf: KdfParams::insecure_for_tests(),
            auto_lock_ms: 1_000,
            settings_path: None,
        };
        let state = AppState::new(password365_app::AppService::new(
            config,
            Box::new(MemorySecretKeyStore::default()),
        ));
        let pw = "long enough password";
        state
            .with(|s| s.create_account(pw.into(), &pw.into()))
            .unwrap();
        let now = Timestamp::now().as_millis();
        assert!(!tick(&state, Timestamp::from_millis(now)));
        assert!(tick(&state, Timestamp::from_millis(now + 60_000)));
        assert!(!state.with(|s| Ok(s.status())).unwrap().unlocked);
    }
}
