//! Merging password histories so no password is ever lost.

use std::collections::BTreeMap;

use password365_crypto::secret::SecretString;
use password365_model::Timestamp;
use password365_model::password_history::{MAX_HISTORY, PasswordHistory};

/// Union of two histories: one entry per password (latest `replaced_at`),
/// newest first, capped at [`MAX_HISTORY`]. Commutative, associative, idempotent.
#[must_use]
pub fn union(a: &PasswordHistory, b: &PasswordHistory) -> PasswordHistory {
    let mut latest: BTreeMap<&str, Timestamp> = BTreeMap::new();
    for entry in a.entries().iter().chain(b.entries()) {
        let slot = latest
            .entry(entry.password.expose())
            .or_insert(entry.replaced_at);
        *slot = (*slot).max(entry.replaced_at);
    }
    let mut ordered: Vec<(&str, Timestamp)> = latest.into_iter().collect();
    ordered.sort_by(|(pa, ta), (pb, tb)| tb.cmp(ta).then_with(|| pa.cmp(pb)));
    ordered.truncate(MAX_HISTORY);
    let mut out = PasswordHistory::default();
    for (password, at) in ordered.into_iter().rev() {
        out.record(SecretString::from(password), at);
    }
    out
}

/// `history` minus any entry equal to `current` (the password in use is not "previous").
#[must_use]
pub fn without(history: &PasswordHistory, current: Option<&SecretString>) -> PasswordHistory {
    let mut out = PasswordHistory::default();
    for entry in history.entries().iter().rev() {
        if Some(&entry.password) != current {
            out.record(entry.password.clone(), entry.replaced_at);
        }
    }
    out
}

/// `true` when `password` appears anywhere in `history`.
#[must_use]
pub fn contains(history: &PasswordHistory, password: &SecretString) -> bool {
    history.entries().iter().any(|e| e.password == *password)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(entries: &[(&str, i64)]) -> PasswordHistory {
        let mut h = PasswordHistory::default();
        for (p, t) in entries.iter().rev() {
            h.record((*p).into(), Timestamp::from_millis(*t));
        }
        h
    }

    fn flat(h: &PasswordHistory) -> Vec<(String, i64)> {
        h.entries()
            .iter()
            .map(|e| (e.password.expose().to_owned(), e.replaced_at.as_millis()))
            .collect()
    }

    #[test]
    fn union_dedupes_by_password_keeping_the_latest_time() {
        let a = history(&[("b", 5), ("a", 1)]);
        let b = history(&[("c", 9), ("a", 3)]);
        let merged = union(&a, &b);
        assert_eq!(
            flat(&merged),
            vec![("c".into(), 9), ("b".into(), 5), ("a".into(), 3)]
        );
        assert_eq!(merged, union(&b, &a));
        assert_eq!(union(&merged, &merged), merged);
    }

    #[test]
    fn equal_times_are_ordered_deterministically() {
        let merged = union(&history(&[("y", 1)]), &history(&[("x", 1)]));
        assert_eq!(flat(&merged), vec![("x".into(), 1), ("y".into(), 1)]);
    }

    #[test]
    fn union_is_capped() {
        let names: Vec<String> = (0..40).map(|i| format!("p{i}")).collect();
        let (low, high) = names.split_at(20);
        let side = |list: &[String], offset: i64| {
            let refs: Vec<(&str, i64)> = list
                .iter()
                .zip(offset..)
                .map(|(p, t)| (p.as_str(), t))
                .collect();
            history(&refs)
        };
        let merged = union(&side(low, 0), &side(high, 20));
        assert_eq!(merged.len(), MAX_HISTORY);
        assert_eq!(merged.entries()[0].password.expose(), "p39");
    }

    #[test]
    fn without_drops_the_current_password_only() {
        let h = history(&[("b", 2), ("a", 1)]);
        assert_eq!(flat(&without(&h, Some(&"b".into()))), vec![("a".into(), 1)]);
        assert_eq!(without(&h, None), h);
    }

    #[test]
    fn contains_matches_by_value() {
        let h = history(&[("a", 1)]);
        assert!(contains(&h, &"a".into()));
        assert!(!contains(&h, &"b".into()));
    }
}
