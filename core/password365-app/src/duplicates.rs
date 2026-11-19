//! Removing logins stored twice (same site, username and password), for vaults that
//! imported the same accounts from two sources.

use std::collections::HashMap;

use password365_model::{Item, ItemId, ItemKind};

use crate::onepux_import::login_keys;
use crate::{AppService, Result};

/// How much an item holds beyond site, username and password; the richest copy is kept.
fn richness(item: &Item) -> usize {
    let d = &item.details;
    usize::from(d.totp.is_some())
        + usize::from(d.notes.is_some())
        + d.custom_fields.len()
        + d.attachments.len()
}

impl AppService {
    /// Moves to Recently Deleted every login that repeats another with the same site, username
    /// and password, keeping the one with most data (then the oldest). Returns how many moved.
    pub fn remove_duplicate_logins(&mut self) -> Result<usize> {
        let candidates: Vec<(ItemId, (String, String))> = self
            .session()?
            .items()
            .into_iter()
            .filter(|s| s.kind == ItemKind::Login)
            .filter_map(|s| {
                login_keys(&s.overview.urls, s.overview.username.as_deref())
                    .into_iter()
                    .next()
                    .map(|key| (s.id, key))
            })
            .collect();
        let mut by_login: HashMap<(String, String), Vec<ItemId>> = HashMap::new();
        for (id, key) in candidates {
            by_login.entry(key).or_default().push(id);
        }
        let mut doomed = Vec::new();
        for ids in by_login.into_values().filter(|ids| ids.len() > 1) {
            let mut by_password: HashMap<Option<String>, Vec<Item>> = HashMap::new();
            for id in ids {
                let item = self.session()?.item(id)?;
                let password = item
                    .details
                    .password
                    .as_ref()
                    .map(|p| p.expose().to_owned());
                by_password.entry(password).or_default().push(item);
            }
            for mut group in by_password.into_values().filter(|g| g.len() > 1) {
                group.sort_by_key(|i| (std::cmp::Reverse(richness(i)), i.created_at, i.id));
                doomed.extend(group.into_iter().skip(1).map(|i| i.id));
            }
        }
        for id in &doomed {
            self.session()?.delete_item(*id)?;
        }
        Ok(doomed.len())
    }
}

#[cfg(test)]
mod tests {
    use password365_crypto::secret::SecretString;

    use crate::item_input::ItemInput;
    use crate::service::tests::with_account;

    fn login(title: &str, url: &str, user: &str, pw: &str, notes: Option<&str>) -> ItemInput {
        ItemInput {
            title: title.into(),
            urls: vec![url.into()],
            username: Some(user.into()),
            password: Some(SecretString::from(pw.to_owned())),
            notes: notes.map(|n| SecretString::from(n.to_owned())),
            ..ItemInput::default()
        }
    }

    #[test]
    fn repeated_logins_go_to_recently_deleted_keeping_the_richest() {
        let mut fx = with_account();
        fx.app
            .create_item(&login("A", "https://a.com/x", "Ann", "pw", None))
            .unwrap();
        fx.app
            .create_item(&login(
                "a.com",
                "https://a.com",
                "ann",
                "pw",
                Some("keep me"),
            ))
            .unwrap();
        fx.app
            .create_item(&login("a.com", "https://a.com", "ann", "other", None))
            .unwrap();
        fx.app
            .create_item(&login("B", "https://b.com", "bob", "pw", None))
            .unwrap();
        assert_eq!(fx.app.remove_duplicate_logins().unwrap(), 1);
        let left = fx.app.items("", None).unwrap();
        assert_eq!(left.len(), 3);
        assert_eq!(fx.app.deleted_items().unwrap().len(), 1);
        assert_eq!(fx.app.remove_duplicate_logins().unwrap(), 0);
        let kept = left
            .iter()
            .find(|i| i.title == "a.com" && i.username.as_deref() == Some("ann"))
            .unwrap();
        assert!(fx.app.item(&kept.id).is_ok());
    }
}
