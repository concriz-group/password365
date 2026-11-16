//! Watchtower (1Password's security report), computed on this device only:
//! weak and reused passwords, logins on unencrypted (http) websites and
//! items that expire soon, plus an overall score from 0 to 1000.
//!
//! Breach checks need the internet and are not done here.

use std::collections::HashMap;

use password365_crypto::secret::SecretString;
use password365_model::{Item, Timestamp};
use serde::Serialize;

use crate::AppService;
use crate::Result;
use crate::password_policy::password_strength;

/// zxcvbn scores below this are weak (1Password's "weak passwords").
const WEAK_BELOW: u8 = 3;
/// Items expiring within this many days are listed.
const EXPIRING_DAYS: i64 = 30;
/// Field labels holding an expiry date.
const EXPIRY_LABELS: [&str; 2] = ["expiry date", "expires"];
const MS_PER_DAY: i64 = 24 * 60 * 60 * 1000;

/// An item a finding points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemRef {
    /// Item id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Category id.
    pub kind: String,
}

/// An item with an expiry date close by or past.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Expiring {
    /// The item.
    pub item: ItemRef,
    /// The date as stored (`2029-04` or `2029-04-30`).
    pub date: String,
    /// Already past.
    pub expired: bool,
}

/// How many passwords fall in each strength band (the strength bar).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct StrengthCounts {
    /// zxcvbn 4.
    pub strong: usize,
    /// zxcvbn 3.
    pub good: usize,
    /// zxcvbn 0–2.
    pub weak: usize,
}

/// The whole report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WatchtowerDto {
    /// 0–1000, `None` when no item has a password.
    pub score: Option<u32>,
    /// `excellent`, `very_good`, `good`, `fair` or `poor`.
    pub rating: Option<&'static str>,
    /// Strength bands.
    pub strength: StrengthCounts,
    /// Weak passwords.
    pub weak: Vec<ItemRef>,
    /// Groups of items sharing one password.
    pub reused: Vec<Vec<ItemRef>>,
    /// Logins with an `http://` website.
    pub unsecured: Vec<ItemRef>,
    /// Expired or expiring within 30 days.
    pub expiring: Vec<Expiring>,
}

/// Builds the report for `items` (the caller leaves archived items out).
#[must_use]
pub fn report(items: &[Item], now: Timestamp) -> WatchtowerDto {
    let mut strength = StrengthCounts::default();
    let mut weak = Vec::new();
    let mut points = 0u32;
    let mut by_password: HashMap<&str, Vec<ItemRef>> = HashMap::new();
    let with_password: Vec<(&Item, &SecretString)> = items
        .iter()
        .filter_map(|i| i.details.password.as_ref().map(|p| (i, p)))
        .collect();
    for &(item, password) in &with_password {
        by_password
            .entry(password.expose())
            .or_default()
            .push(item_ref(item));
    }
    for &(item, password) in &with_password {
        let score = password_strength(password).score;
        let reused = by_password[password.expose()].len() > 1;
        match score {
            4 => strength.strong += 1,
            3 => strength.good += 1,
            _ => strength.weak += 1,
        }
        if score < WEAK_BELOW {
            weak.push(item_ref(item));
        }
        // A reused password counts as weak in the score, however long it is.
        points += if reused { 0 } else { u32::from(score) };
    }
    let mut reused: Vec<Vec<ItemRef>> = by_password.into_values().filter(|g| g.len() > 1).collect();
    reused.sort_by(|a, b| {
        b.len()
            .cmp(&a.len())
            .then_with(|| a[0].title.cmp(&b[0].title))
    });
    let score = u32::try_from(with_password.len())
        .ok()
        .filter(|&n| n > 0)
        .map(|n| points * 250 / n);
    WatchtowerDto {
        score,
        rating: score.map(rating),
        strength,
        weak,
        reused,
        unsecured: items
            .iter()
            .filter(|i| {
                i.overview
                    .urls
                    .iter()
                    .any(|u| u.to_ascii_lowercase().starts_with("http://"))
            })
            .map(item_ref)
            .collect(),
        expiring: items.iter().filter_map(|i| expiring(i, now)).collect(),
    }
}

fn rating(score: u32) -> &'static str {
    match score {
        900.. => "excellent",
        750.. => "very_good",
        600.. => "good",
        400.. => "fair",
        _ => "poor",
    }
}

fn item_ref(item: &Item) -> ItemRef {
    ItemRef {
        id: item.id.to_string(),
        title: item.overview.title.clone(),
        kind: item.kind.to_string(),
    }
}

fn expiring(item: &Item, now: Timestamp) -> Option<Expiring> {
    let field = item.details.custom_fields.iter().find(|f| {
        EXPIRY_LABELS
            .iter()
            .any(|l| f.label.eq_ignore_ascii_case(l))
    })?;
    let date = field.value.expose().trim();
    let last_day = last_valid_day(date)?;
    let today = now.as_millis().div_euclid(MS_PER_DAY);
    (last_day - today <= EXPIRING_DAYS).then(|| Expiring {
        item: item_ref(item),
        date: date.to_owned(),
        expired: last_day < today,
    })
}

/// Days since 1970-01-01 of the last day a `YYYY-MM` or `YYYY-MM-DD` date is valid.
fn last_valid_day(date: &str) -> Option<i64> {
    let mut parts = date.split('-').map(|p| p.parse::<i64>().ok());
    let year = parts.next()??;
    let month = parts.next()??;
    let day = match parts.next() {
        Some(day) => day?,
        None => days_in_month(year, month)?,
    };
    if parts.next().is_some() || !(1..=days_in_month(year, month)?).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

fn days_in_month(year: i64, month: i64) -> Option<i64> {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    })
}

/// Howard Hinnant's `days_from_civil`.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

impl AppService {
    /// The Watchtower report for every item outside the Archive.
    pub fn watchtower(&mut self) -> Result<WatchtowerDto> {
        let session = self.session()?;
        let items = session
            .items()
            .into_iter()
            .filter(|s| crate::service::active(s))
            .map(|s| session.item(s.id))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(report(&items, Timestamp::now()))
    }
}

#[cfg(test)]
#[path = "watchtower_tests.rs"]
mod tests;
