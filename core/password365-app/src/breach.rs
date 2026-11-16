//! Watchtower › Vulnerable passwords: asks Have I Been Pwned whether a
//! password appeared in a data breach, without revealing it. Only the first
//! five characters of its SHA-1 hash leave the device (k-anonymity), the
//! answer is padded with fake entries, and nothing runs unless the user
//! turned it on (Settings › Privacy).

use std::collections::HashMap;
use std::time::Duration;

use password365_model::ItemKind;
use serde::Serialize;
use sha1::{Digest, Sha1};

use crate::watchtower::ItemRef;
use crate::{AppError, AppService, Result};

const RANGE_URL: &str = "https://api.pwnedpasswords.com/range/";

/// Fetches the hash suffixes (`SUFFIX:COUNT` lines) for a 5-character prefix.
pub trait RangeSource {
    /// The response body for `prefix`.
    fn range(&self, prefix: &str) -> Result<String>;
}

/// The real service, over HTTPS.
pub struct Hibp;

impl RangeSource for Hibp {
    fn range(&self, prefix: &str) -> Result<String> {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .new_agent();
        agent
            .get(format!("{RANGE_URL}{prefix}"))
            .header("Add-Padding", "true")
            .header("User-Agent", "Password365")
            .call()
            .and_then(|mut r| r.body_mut().read_to_string())
            .map_err(|e| AppError::Io(format!("haveibeenpwned.com: {e}")))
    }
}

/// `(prefix, suffix)` of a password's upper-case hex SHA-1.
#[must_use]
pub fn hash_parts(password: &str) -> (String, String) {
    let digest = Sha1::digest(password.as_bytes());
    let hex = digest.iter().fold(String::with_capacity(40), |mut hex, b| {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02X}");
        hex
    });
    let (prefix, suffix) = hex.split_at(5);
    (prefix.to_owned(), suffix.to_owned())
}

/// How often `suffix` appears in a range response (0: not found or a padding entry).
#[must_use]
pub fn count_in(response: &str, suffix: &str) -> u64 {
    response
        .lines()
        .filter_map(|line| line.trim().split_once(':'))
        .find(|(s, _)| s.eq_ignore_ascii_case(suffix))
        .and_then(|(_, n)| n.trim().parse().ok())
        .unwrap_or(0)
}

/// A login whose password appeared in breaches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BreachedDto {
    /// The item.
    pub item: ItemRef,
    /// How many times the password was seen.
    pub count: u64,
}

/// Logins to check: each with its password's hash parts (never the password).
pub type Candidates = Vec<(ItemRef, (String, String))>;

/// Asks `source` about each candidate, one request per distinct prefix.
pub fn check(candidates: Candidates, source: &dyn RangeSource) -> Result<Vec<BreachedDto>> {
    let mut ranges: HashMap<String, String> = HashMap::new();
    let mut found = Vec::new();
    for (item, (prefix, suffix)) in candidates {
        if !ranges.contains_key(&prefix) {
            let body = source.range(&prefix)?;
            ranges.insert(prefix.clone(), body);
        }
        let count = count_in(&ranges[&prefix], &suffix);
        if count > 0 {
            found.push(BreachedDto { item, count });
        }
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.count));
    Ok(found)
}

impl AppService {
    /// Logins whose password appeared in data breaches. Refused unless
    /// Settings › Privacy allows it.
    pub fn breached_passwords(&mut self, source: &dyn RangeSource) -> Result<Vec<BreachedDto>> {
        check(self.breach_candidates()?, source)
    }

    /// The logins to check and their hash parts (quick, with the vault open;
    /// the slow network part is [`check`]). Refused unless allowed.
    pub fn breach_candidates(&mut self) -> Result<Candidates> {
        if !self.settings().check_breached_passwords {
            return Err(AppError::Validation(
                "checking passwords online is turned off".into(),
            ));
        }
        let session = self.session()?;
        let mut logins = Vec::new();
        for summary in session
            .items()
            .into_iter()
            .filter(|s| crate::service::active(s))
        {
            if !matches!(summary.kind, ItemKind::Login | ItemKind::Password) {
                continue;
            }
            let item = session.item(summary.id)?;
            if let Some(password) = item.details.password.as_ref().filter(|p| !p.is_empty()) {
                let reference = ItemRef {
                    id: item.id.to_string(),
                    title: item.overview.title.clone(),
                    kind: item.kind.as_str().to_owned(),
                };
                logins.push((reference, hash_parts(password.expose())));
            }
        }
        Ok(logins)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::item_input::ItemInput;
    use crate::service::tests::{login, with_account};
    use crate::settings::Settings;

    struct Fake(RefCell<Vec<String>>);

    impl RangeSource for Fake {
        fn range(&self, prefix: &str) -> Result<String> {
            self.0.borrow_mut().push(prefix.to_owned());
            let (_, suffix) = hash_parts("password");
            Ok(format!(
                "0000000000000000000000000000000000A:0\r\n{suffix}:52256179\r\n"
            ))
        }
    }

    #[test]
    fn hashes_and_reads_ranges() {
        let (prefix, suffix) = hash_parts("password");
        assert_eq!(prefix, "5BAA6");
        assert_eq!(suffix, "1E4C9B93F3F0682250B6CF8331B7EE68FD8");
        assert_eq!(
            count_in("1E4C9B93F3F0682250B6CF8331B7EE68FD8:42\nABC:1", &suffix),
            42
        );
        assert_eq!(count_in("ABC:0", "ABC"), 0, "padding");
        assert_eq!(count_in("garbage", "ABC"), 0);
    }

    #[test]
    fn finds_breached_logins_only_when_allowed_and_asks_each_prefix_once() {
        let mut fx = with_account();
        for (title, pw) in [
            ("A", "password"),
            ("B", "password"),
            ("C", "a much longer unique one"),
        ] {
            fx.app.create_item(&login(title, pw)).unwrap();
        }
        fx.app
            .create_item(&ItemInput {
                kind: Some("note".into()),
                title: "N".into(),
                ..ItemInput::default()
            })
            .unwrap();
        let fake = Fake(RefCell::new(Vec::new()));
        assert_eq!(
            fx.app.breached_passwords(&fake).unwrap_err().code(),
            "validation"
        );
        assert!(fake.0.borrow().is_empty(), "nothing leaves while off");
        let on = Settings {
            check_breached_passwords: true,
            auto_lock_minutes: 10,
            ..fx.app.settings()
        };
        fx.app.update_settings(on).unwrap();
        let found = fx.app.breached_passwords(&fake).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].count, 52_256_179);
        assert_eq!(fake.0.borrow().len(), 2, "one request per distinct prefix");
        assert!(fake.0.borrow().iter().all(|p| p.len() == 5));
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Talks to the real service (run by hand: `cargo test -- --ignored live`).
    #[test]
    #[ignore = "uses the internet"]
    fn live_range_answers_for_a_known_password() {
        let (prefix, suffix) = hash_parts("password");
        let body = Hibp.range(&prefix).unwrap();
        assert!(count_in(&body, &suffix) > 1_000_000);
    }
}
