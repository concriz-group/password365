//! Watchtower for the phone: the same on-device security report as the desktop
//! (weak and reused passwords, http sites, expiring items).

use password365_app::watchtower::{ItemRef, WatchtowerDto};

use crate::{Password365, Result};

/// An item a finding points at.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WatchItem {
    /// Item id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Category (`login`, `card`, …).
    pub kind: String,
}

impl From<ItemRef> for WatchItem {
    fn from(i: ItemRef) -> Self {
        Self {
            id: i.id,
            title: i.title,
            kind: i.kind,
        }
    }
}

/// Items sharing one password.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReusedGroup {
    /// The items.
    pub items: Vec<WatchItem>,
}

/// An item with an expiry date close by or past.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ExpiringItem {
    /// The item.
    pub item: WatchItem,
    /// The date as stored.
    pub date: String,
    /// Already past.
    pub expired: bool,
}

/// The security report.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WatchtowerReport {
    /// 0–1000, `None` when no item has a password.
    pub score: Option<u32>,
    /// `excellent`, `very_good`, `good`, `fair`, `poor` or "".
    pub rating: String,
    /// Strong passwords.
    pub strong: u64,
    /// Good passwords.
    pub good: u64,
    /// Weak passwords (count; the items are in `weak`).
    pub weak_count: u64,
    /// Weak passwords.
    pub weak: Vec<WatchItem>,
    /// Groups of items with the same password.
    pub reused: Vec<ReusedGroup>,
    /// Logins on `http://` sites.
    pub unsecured: Vec<WatchItem>,
    /// Expired or expiring soon.
    pub expiring: Vec<ExpiringItem>,
}

impl From<WatchtowerDto> for WatchtowerReport {
    fn from(d: WatchtowerDto) -> Self {
        Self {
            score: d.score,
            rating: d.rating.unwrap_or_default().to_owned(),
            strong: d.strength.strong as u64,
            good: d.strength.good as u64,
            weak_count: d.strength.weak as u64,
            weak: d.weak.into_iter().map(Into::into).collect(),
            reused: d
                .reused
                .into_iter()
                .map(|g| ReusedGroup {
                    items: g.into_iter().map(Into::into).collect(),
                })
                .collect(),
            unsecured: d.unsecured.into_iter().map(Into::into).collect(),
            expiring: d
                .expiring
                .into_iter()
                .map(|e| ExpiringItem {
                    item: e.item.into(),
                    date: e.date,
                    expired: e.expired,
                })
                .collect(),
        }
    }
}

#[uniffi::export]
impl Password365 {
    /// Logins with no website: `AutoFill` never suggests them by themselves (only by picking
    /// from the list), so the app asks the person to add the site.
    pub fn logins_without_site(&self) -> Result<Vec<WatchItem>> {
        Ok(self
            .app()
            .items("", None)?
            .into_iter()
            .filter(|s| s.kind == "login" && s.urls.iter().all(|u| u.trim().is_empty()))
            .map(|s| WatchItem {
                id: s.id,
                title: s.title,
                kind: s.kind,
            })
            .collect())
    }

    /// The Watchtower report for every item outside the Archive (computed on this device).
    pub fn watchtower(&self) -> Result<WatchtowerReport> {
        Ok(self.app().watchtower()?.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use password365_app::watchtower::StrengthCounts;

    #[test]
    fn the_report_crosses_with_its_counts_and_groups() {
        let item = |id: &str| ItemRef {
            id: id.into(),
            title: id.into(),
            kind: "login".into(),
        };
        let dto = WatchtowerDto {
            score: Some(640),
            rating: Some("good"),
            strength: StrengthCounts {
                strong: 3,
                good: 2,
                weak: 1,
            },
            weak: vec![item("a")],
            reused: vec![vec![item("b"), item("c")]],
            unsecured: vec![],
            expiring: vec![],
        };
        let r = WatchtowerReport::from(dto);
        assert_eq!(
            (r.score, r.rating.as_str(), r.weak_count),
            (Some(640), "good", 1)
        );
        assert_eq!((r.weak[0].id.as_str(), r.reused[0].items.len()), ("a", 2));
    }

    #[test]
    fn logins_without_a_website_are_listed() {
        use crate::tests::{MemoryKeeper, PASSWORD, vault};
        use password365_app::item_input::ItemInput;
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let p = vault(&dir, Arc::new(MemoryKeeper::default()));
        p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
        let add = |title: &str, url: Option<&str>| {
            p.app()
                .create_item(&ItemInput {
                    title: title.into(),
                    urls: url.map(|u| vec![u.to_owned()]).unwrap_or_default(),
                    ..ItemInput::default()
                })
                .unwrap();
        };
        add("No site", None);
        add("Has site", Some("https://a.com"));
        let found = p.logins_without_site().unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "No site");
    }
}
