//! Comparing imported logins with existing vault items before import.

use std::collections::HashSet;

use password365_model::{Item, ItemId, ItemKind};

use crate::ImportedLogin;
use crate::url_key::origin_key;

/// What to do with the imported logins.
#[derive(Debug, Default)]
pub struct ImportPlan {
    /// Not in the vault yet → will be added.
    pub new: Vec<ImportedLogin>,
    /// Same site, username and password already stored → skipped.
    pub identical: Vec<(ImportedLogin, ItemId)>,
    /// Same site and username but a different password → user decides.
    pub changed: Vec<(ImportedLogin, ItemId)>,
    /// Repeated rows inside the import file itself → skipped.
    pub duplicates_in_file: usize,
}

/// Identity of a login: normalized origin + lowercase username.
fn identity(url: Option<&str>, username: Option<&str>) -> (String, String) {
    (
        url.map(origin_key).unwrap_or_default(),
        username.unwrap_or_default().trim().to_lowercase(),
    )
}

/// Classifies every imported login against `existing` items.
#[must_use]
pub fn plan(imported: Vec<ImportedLogin>, existing: &[Item]) -> ImportPlan {
    let mut result = ImportPlan::default();
    let mut seen = HashSet::new();
    for login in imported {
        if login.note_only {
            // Notes have no site or username: the same title and text is already there.
            let same = existing.iter().find(|i| {
                i.kind == ItemKind::SecureNote
                    && i.overview.title == login.title.trim()
                    && i.details.notes == login.notes
            });
            match same {
                Some(item) => result.identical.push((login, item.id)),
                None => result.new.push(login),
            }
            continue;
        }
        let key = identity(login.url.as_deref(), login.username.as_deref());
        if !seen.insert(key.clone()) {
            result.duplicates_in_file += 1;
            continue;
        }
        if key.0.is_empty() {
            // No site: nothing identifies it safely. Only an exact copy (same title,
            // username and password) is recognised; otherwise it is new, never an update.
            match same_without_site(&login, existing) {
                Some(item) => result.identical.push((login, item.id)),
                None => result.new.push(login),
            }
            continue;
        }
        match find_existing(&key, existing) {
            None => result.new.push(login),
            Some(item) if item.details.password == login.password => {
                result.identical.push((login, item.id));
            }
            // An empty password in the file never replaces a stored one.
            Some(item)
                if login
                    .password
                    .as_ref()
                    .is_none_or(|p| p.expose().is_empty()) =>
            {
                result.identical.push((login, item.id));
            }
            Some(item) => result.changed.push((login, item.id)),
        }
    }
    result
}

/// A stored login with the same site (non-empty origin) and username.
fn find_existing<'a>(key: &(String, String), existing: &'a [Item]) -> Option<&'a Item> {
    existing
        .iter()
        .filter(|i| i.kind == ItemKind::Login)
        .find(|item| {
            let username = item.overview.username.as_deref();
            item.overview
                .urls
                .iter()
                .any(|url| &identity(Some(url), username) == key)
        })
}

/// A stored login without a site that is an exact copy of `login`.
fn same_without_site<'a>(login: &ImportedLogin, existing: &'a [Item]) -> Option<&'a Item> {
    let username = login
        .username
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    existing.iter().find(|i| {
        i.kind == ItemKind::Login
            && i.overview.urls.iter().all(|u| origin_key(u).is_empty())
            && i.overview.title == login.title.trim()
            && i.overview
                .username
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_lowercase()
                == username
            && i.details.password == login.password
    })
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview, Timestamp, VaultId};

    use super::*;
    use crate::imported::tests::login;

    fn existing(url: &str, user: &str, pw: &str) -> Item {
        Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("x")
                .unwrap()
                .with_url(url)
                .with_username(user),
            ItemDetails::with_password(pw.into()),
            Timestamp::now(),
        )
    }

    #[test]
    fn classifies_new_identical_and_changed() {
        let stored = vec![
            existing("https://a.com/login", "Alice", "pw"),
            existing("https://b.com", "bob", "old"),
        ];
        let imported = vec![
            login("", Some("https://A.com/other"), Some("alice"), Some("pw")),
            login("", Some("https://b.com/"), Some("bob"), Some("new")),
            login("", Some("https://c.com"), Some("carol"), Some("pw")),
        ];
        let p = plan(imported, &stored);
        assert_eq!(p.identical.len(), 1);
        assert_eq!(p.identical[0].1, stored[0].id);
        assert_eq!(p.changed.len(), 1);
        assert_eq!(p.changed[0].1, stored[1].id);
        assert_eq!(p.new.len(), 1);
        assert_eq!(p.duplicates_in_file, 0);
    }

    #[test]
    fn same_site_different_user_is_new() {
        let stored = vec![existing("https://a.com", "alice", "pw")];
        let p = plan(
            vec![login("", Some("https://a.com"), Some("eve"), Some("pw"))],
            &stored,
        );
        assert_eq!(p.new.len(), 1);
    }

    #[test]
    fn duplicates_inside_file_are_counted_once() {
        let rows = vec![
            login("", Some("https://a.com"), Some("u"), Some("1")),
            login("", Some("https://a.com/x"), Some("U"), Some("2")),
        ];
        let p = plan(rows, &[]);
        assert_eq!(p.new.len(), 1);
        assert_eq!(p.duplicates_in_file, 1);
    }

    #[test]
    fn notes_match_notes_with_the_same_title_and_text() {
        let note = |text: &str| {
            let mut n = login("Wi-Fi", None, None, None);
            n.note_only = true;
            n.notes = Some(text.into());
            n
        };
        let stored = note("key")
            .into_item(VaultId::new(), Timestamp::now())
            .unwrap();
        let p = plan(vec![note("key"), note("other")], &[stored]);
        assert_eq!(p.identical.len(), 1);
        assert_eq!(p.new.len(), 1);
    }

    #[test]
    fn a_login_without_a_site_matches_only_an_exact_copy() {
        let mut item = existing("https://ignored", "u", "pw");
        item.overview.urls.clear();
        item.overview.title = "t".into();
        let p = plan(
            vec![login("t", None, Some("u"), Some("pw"))],
            std::slice::from_ref(&item),
        );
        assert_eq!(p.identical.len(), 1);
        // Different password, title or no username: new, never an update of `item`.
        let p = plan(
            vec![
                login("t", None, Some("u"), Some("other")),
                login("bank", None, None, Some("pw2")),
            ],
            &[item],
        );
        assert_eq!(p.new.len(), 2);
        assert!(p.changed.is_empty());
    }

    #[test]
    fn a_row_without_site_or_username_never_updates_an_item_without_them() {
        let mut item = existing("https://ignored", "", "secret");
        item.overview.urls.clear();
        item.overview.username = None;
        let p = plan(vec![login("x", None, None, Some("attacker"))], &[item]);
        assert!(p.changed.is_empty());
        assert_eq!(p.new.len(), 1);
    }

    #[test]
    fn an_empty_password_never_replaces_a_stored_one() {
        let stored = vec![existing("https://a.com", "u", "pw")];
        let p = plan(
            vec![login("", Some("https://a.com"), Some("u"), Some(""))],
            &stored,
        );
        assert!(p.changed.is_empty());
        let p = plan(
            vec![login("", Some("https://a.com"), Some("u"), None)],
            &stored,
        );
        assert!(p.changed.is_empty());
    }

    #[test]
    fn only_logins_are_matched() {
        let mut card = existing("https://a.com", "u", "pw");
        card.kind = ItemKind::CreditCard;
        let p = plan(
            vec![login("", Some("https://a.com"), Some("u"), Some("new"))],
            &[card],
        );
        assert_eq!(p.new.len(), 1);
    }
}
