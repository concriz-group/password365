//! Millisecond-precision UTC timestamps.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch (UTC).
///
/// Business logic receives timestamps as parameters instead of reading the
/// clock itself, which keeps it deterministic and testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Builds a timestamp from milliseconds since the epoch.
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// Milliseconds since the epoch.
    #[must_use]
    pub const fn as_millis(self) -> i64 {
        self.0
    }

    /// Current wall-clock time.
    #[must_use]
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        Self(millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millis_round_trip() {
        assert_eq!(Timestamp::from_millis(42).as_millis(), 42);
    }

    #[test]
    fn now_is_after_2025() {
        assert!(Timestamp::now() > Timestamp::from_millis(1_735_689_600_000));
    }

    #[test]
    fn ordering_follows_time() {
        assert!(Timestamp::from_millis(1) < Timestamp::from_millis(2));
    }
}
