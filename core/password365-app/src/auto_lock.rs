//! Idle-timeout bookkeeping for automatic locking.

use password365_model::Timestamp;

/// Default idle time before locking (10 minutes).
pub const DEFAULT_TIMEOUT_MS: i64 = 10 * 60 * 1000;

/// Tracks user activity and decides when the vault must lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoLock {
    timeout_ms: i64,
    last_activity: Timestamp,
}

impl AutoLock {
    /// Creates a tracker; a timeout ≤ 0 disables auto-lock.
    #[must_use]
    pub fn new(timeout_ms: i64, now: Timestamp) -> Self {
        Self {
            timeout_ms,
            last_activity: now,
        }
    }

    /// Records user activity.
    pub fn touch(&mut self, now: Timestamp) {
        self.last_activity = self.last_activity.max(now);
    }

    /// `true` once the idle time exceeds the timeout.
    #[must_use]
    pub fn is_expired(&self, now: Timestamp) -> bool {
        self.timeout_ms > 0 && now.as_millis() - self.last_activity.as_millis() >= self.timeout_ms
    }

    /// Configured timeout in milliseconds.
    #[must_use]
    pub const fn timeout_ms(&self) -> i64 {
        self.timeout_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: Timestamp = Timestamp::from_millis(1_000);

    #[test]
    fn expires_after_timeout() {
        let lock = AutoLock::new(100, T0);
        assert!(!lock.is_expired(Timestamp::from_millis(1_099)));
        assert!(lock.is_expired(Timestamp::from_millis(1_100)));
    }

    #[test]
    fn activity_postpones_expiry() {
        let mut lock = AutoLock::new(100, T0);
        lock.touch(Timestamp::from_millis(1_090));
        assert!(!lock.is_expired(Timestamp::from_millis(1_150)));
    }

    #[test]
    fn touch_ignores_clock_going_backwards() {
        let mut lock = AutoLock::new(100, T0);
        lock.touch(Timestamp::from_millis(500));
        assert!(lock.is_expired(Timestamp::from_millis(1_100)));
    }

    #[test]
    fn zero_timeout_disables() {
        let lock = AutoLock::new(0, T0);
        assert!(!lock.is_expired(Timestamp::from_millis(i64::MAX / 2)));
        assert_eq!(lock.timeout_ms(), 0);
    }
}
