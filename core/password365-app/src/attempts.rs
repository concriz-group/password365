//! Wrong master passwords make the next try wait, longer each time, so nobody
//! at this computer (or a program talking to the browser bridge) can guess
//! quickly. Stored next to the settings so quitting the app does not reset it.
//! Copies of the vault taken elsewhere are protected by the Secret Key and
//! Argon2id instead: no app can slow down someone else's computer.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{AppError, Result};

const FILE: &str = "unlock-attempts.json";

/// Seconds to wait after the n-th wrong password in a row (the first two are free).
const WAITS: [i64; 8] = [0, 0, 5, 15, 30, 60, 120, 300];
/// Wait after every further wrong password.
const LONGEST: i64 = 900;
/// After this many wrong passwords in a row the device forgets the Secret
/// Key: the next unlock needs it typed from the Emergency Kit.
pub const FORGET_SECRET_KEY_AFTER: u32 = 10;

/// Wrong passwords in a row and when the next try is allowed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Attempts {
    failures: u32,
    wait_until_ms: i64,
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
        let left = self.wait_until_ms - now_ms;
        if left > 0 {
            let seconds = u64::try_from((left + 999) / 1000).unwrap_or(1);
            return Err(AppError::TooManyAttempts(seconds));
        }
        Ok(())
    }

    /// Another wrong password.
    pub fn failed(&mut self, now_ms: i64) {
        self.failures = self.failures.saturating_add(1);
        self.wait_until_ms = now_ms + wait_after(self.failures) * 1000;
    }

    /// The right password: start over.
    pub fn succeeded(&mut self) {
        *self = Self::default();
    }

    /// Ends the current wait (tests only: time does not pass in them).
    #[cfg(test)]
    pub(crate) fn skip_wait(&mut self) {
        self.wait_until_ms = 0;
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

pub(crate) fn load(settings_path: Option<&Path>) -> Attempts {
    path(settings_path)
        .and_then(|p| crate::json_file::read(&p))
        .unwrap_or_default()
}

pub(crate) fn save(settings_path: Option<&Path>, attempts: &Attempts) {
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
        assert_eq!(waits, [0, 0, 0, 5, 15, 30, 60, 120, 300, 900, 900]);
        let mut a = Attempts::default();
        a.failed(0);
        a.failed(0);
        assert!(a.check(0).is_ok(), "two free tries");
        a.failed(10_000);
        assert_eq!(a.check(10_000).unwrap_err(), AppError::TooManyAttempts(5));
        assert_eq!(a.check(14_500).unwrap_err(), AppError::TooManyAttempts(1));
        assert!(a.check(15_000).is_ok());
        a.succeeded();
        assert_eq!(a.failures(), 0);
    }
}
