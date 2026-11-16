//! Comparing imported logins with existing vault items before import.

use std::collections::HashSet;

use password365_model::{Item, ItemId};

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
                i.kind == password365_model::ItemKind::SecureNote
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
        match find_existing(&key, existing) {
            None => result.new.push(login),
            Some(item) if item.details.password == login.password => {
                result.identical.push((login, item.id));
            }
            Some(item) => result.changed.push((login, item.id)),
        }
    }
    result
}

fn find_existing<'a>(key: &(String, String), existing: &'a [Item]) -> Option<&'a Item> {
    existing.iter().find(|item| {
        let username = item.overview.username.as_deref();
        let urls: Vec<Option<&str>> = if item.overview.urls.is_empty() {
            vec![None]
        } else {
            item.overview
                .urls
                .iter()
                .map(|u| Some(u.as_str()))
                .collect()
        };
        urls.into_iter().any(|url| &identity(url, username) == key)
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
    fn items_without_urls_match_logins_without_urls() {
        let mut item = existing("https://ignored", "u", "pw");
        item.overview.urls.clear();
        let p = plan(vec![login("t", None, Some("u"), Some("pw"))], &[item]);
        assert_eq!(p.identical.len(), 1);
    }
}
