//! Previous passwords of an item, newest first.

use password365_crypto::secret::SecretString;
use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// Maximum number of previous passwords kept per item.
pub const MAX_HISTORY: usize = 32;

/// A password that was replaced at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordHistoryEntry {
    /// The previous password.
    pub password: SecretString,
    /// When it was replaced.
    pub replaced_at: Timestamp,
}

/// Bounded list of previous passwords (newest first).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PasswordHistory(Vec<PasswordHistoryEntry>);

impl PasswordHistory {
    /// Records a replaced password, dropping the oldest beyond [`MAX_HISTORY`].
    pub fn record(&mut self, password: SecretString, replaced_at: Timestamp) {
        if password.is_empty() {
            return;
        }
        self.0.insert(
            0,
            PasswordHistoryEntry {
                password,
                replaced_at,
            },
        );
        self.0.truncate(MAX_HISTORY);
    }

    /// Entries, newest first.
    #[must_use]
    pub fn entries(&self) -> &[PasswordHistoryEntry] {
        &self.0
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` when no previous password is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_newest_first() {
        let mut h = PasswordHistory::default();
        h.record("old".into(), Timestamp::from_millis(1));
        h.record("older-replaced-later".into(), Timestamp::from_millis(2));
        assert_eq!(h.len(), 2);
        assert_eq!(h.entries()[0].replaced_at, Timestamp::from_millis(2));
    }

    #[test]
    fn empty_passwords_are_not_recorded() {
        let mut h = PasswordHistory::default();
        h.record("".into(), Timestamp::from_millis(1));
        assert!(h.is_empty());
    }

    #[test]
    fn history_is_bounded() {
        let mut h = PasswordHistory::default();
        for i in 0..i64::try_from(MAX_HISTORY + 5).unwrap() {
            h.record(format!("p{i}").into(), Timestamp::from_millis(i));
        }
        assert_eq!(h.len(), MAX_HISTORY);
        // the newest survives
        assert_eq!(
            h.entries()[0].password.expose(),
            format!("p{}", MAX_HISTORY + 4)
        );
    }
}
