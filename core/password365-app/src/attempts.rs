//! Wrong master passwords make the next try wait, longer each time, so nobody
//! at this computer (or a program talking to the browser bridge) can guess
//! quickly. Stored next to the settings so quitting the app does not reset it.
//! Copies of the vault taken elsewhere are protected by the Secret Key and
//! Argon2id instead: no app can slow down someone else's computer.
//!
//! This is a speed bump, never a reason to lose data: wrong passwords never
//! delete anything (an earlier design forgot the Secret Key after ten, which
//! let anyone at the screen lock the owner out of a vault without an
//! Emergency Kit). While the app runs, the wait is also kept on the monotonic
//! clock, so moving the computer's clock forward does not end it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{AppError, Result};

const FILE: &str = "unlock-attempts.json";

/// Seconds to wait after the n-th wrong password in a row (the first two are free).
const WAITS: [i64; 9] = [0, 0, 5, 15, 30, 60, 120, 300, 900];
/// Wait after every further wrong password.
const LONGEST: i64 = 3600;

/// Wrong passwords in a row and when the next try is allowed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Attempts {
    failures: u32,
    wait_until_ms: i64,
    /// The same wait on the monotonic clock (this run of the app only).
    #[serde(skip)]
    wait_until: Option<Instant>,
}

/// Seconds to wait after `failures` wrong passwords in a row.
#[must_use]
pub fn wait_after(failures: u32) -> i64 {
    usize::try_from(failures)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|i| WAITS.get(i).copied())
        .unwrap_or(if failures == 0 { 0 } else { LONGEST })
}

impl Attempts {
    /// `Err(TooManyAttempts)` while the wait is not over.
    pub fn check(&self, now_ms: i64) -> Result<()> {
        let monotonic = self.wait_until.map_or(0, |until| {
            i64::try_from(until.saturating_duration_since(Instant::now()).as_millis())
                .unwrap_or(i64::MAX)
        });
        let left = (self.wait_until_ms - now_ms).max(monotonic);
        if left > 0 {
            let seconds = u64::try_from((left + 999) / 1000).unwrap_or(1);
            return Err(AppError::TooManyAttempts(seconds));
        }
        Ok(())
    }

    /// Another wrong password.
    pub fn failed(&mut self, now_ms: i64) {
        self.failures = self.failures.saturating_add(1);
        let wait = wait_after(self.failures);
        self.wait_until_ms = now_ms + wait * 1000;
        self.wait_until =
            Some(Instant::now() + Duration::from_secs(u64::try_from(wait).unwrap_or(0)));
    }

    /// The right password: start over.
    pub fn succeeded(&mut self) {
        *self = Self::default();
    }

    /// Ends the current wait (tests only: time does not pass in them).
    #[cfg(test)]
    pub(crate) fn skip_wait(&mut self) {
        self.wait_until_ms = 0;
        self.wait_until = None;
    }

    /// Wrong passwords in a row so far.
    #[must_use]
    pub const fn failures(&self) -> u32 {
        self.failures
    }
}

pub(crate) fn path(settings_path: Option<&Path>) -> Option<PathBuf> {
    settings_path.map(|p| p.with_file_name(FILE))
}

/// The attempts saved beside `settings_path` (none yet: a fresh count).
#[must_use]
pub fn load(settings_path: Option<&Path>) -> Attempts {
    path(settings_path)
        .and_then(|p| crate::json_file::read(&p))
        .unwrap_or_default()
}

/// Saves the attempts beside `settings_path` (best effort).
pub fn save(settings_path: Option<&Path>, attempts: &Attempts) {
    if let Some(p) = path(settings_path) {
        let _ = crate::json_file::write(&p, attempts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_grow_with_each_wrong_password() {
        let waits: Vec<i64> = (0..=10).map(wait_after).collect();
        assert_eq!(waits, [0, 0, 0, 5, 15, 30, 60, 120, 300, 900, 3600]);
        let mut a = Attempts::default();
        a.failed(0);
        a.failed(0);
        assert!(a.check(0).is_ok(), "two free tries");
        a.failed(10_000);
        a.wait_until = None; // (wall clock only here: tests do not sleep)
        assert_eq!(a.check(10_000).unwrap_err(), AppError::TooManyAttempts(5));
        assert_eq!(a.check(14_500).unwrap_err(), AppError::TooManyAttempts(1));
        assert!(a.check(15_000).is_ok());
        a.succeeded();
        assert_eq!(a.failures(), 0);
    }

    #[test]
    fn moving_the_clock_forward_does_not_end_the_wait() {
        let mut a = Attempts::default();
        for _ in 0..3 {
            a.failed(0);
        }
        // An hour later by the wall clock, but only moments by the monotonic one.
        assert!(matches!(
            a.check(3_600_000),
            Err(AppError::TooManyAttempts(_))
        ));
    }
}
