//! Reviewing logins that look like the same account stored more than once, and merging them.
//!
//! A group is the logins with the same username on sites of one account family (`amazon.es`,
//! `www.amazon.es` and `amazon.com.br` are one family; any other site is its own). Nothing is
//! decided here: the person sees each group, picks the login to keep, and the others' websites are
//! added to it before they go to Recently Deleted.

use std::collections::{BTreeMap, HashMap};

use password365_match::{Site, family};
use password365_model::{Item, ItemId, ItemKind};
use serde::Serialize;

use crate::service::parse_id;
use crate::{AppError, AppService, Result};

/// One login of a group (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DuplicateMember {
    /// Item id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Websites.
    pub urls: Vec<String>,
    /// Last change (ms since epoch).
    pub updated_at: i64,
    /// Members with the same number have the same password (the passwords themselves stay hidden).
    pub password_group: usize,
    /// Has a one-time password.
    pub has_totp: bool,
    /// Has notes.
    pub has_notes: bool,
    /// Passwords it replaced earlier.
    pub history: usize,
}

/// Logins that look like the same account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DuplicateGroup {
    /// The site the group is about (the most common one among the members).
    pub site: String,
    /// The username they share.
    pub username: String,
    /// The members, the suggested one to keep first.
    pub members: Vec<DuplicateMember>,
    /// Every member has the same password.
    pub same_password: bool,
}

fn richness(item: &Item) -> usize {
    let d = &item.details;
    usize::from(d.totp.is_some())
        + usize::from(d.notes.is_some())
        + d.custom_fields.len()
        + d.attachments.len()
        + d.password_history.len()
}

/// The key of the group a login belongs to: its family and username, if it has a website.
fn key_of(urls: &[String], username: Option<&str>) -> Option<(String, String)> {
    let site = urls.iter().find_map(|u| Site::parse(u))?;
    // IP addresses and single-label hosts are not "families" of anything.
    Some((
        family(&site),
        username.unwrap_or_default().trim().to_lowercase(),
    ))
}

impl AppService {
    /// The groups of logins that look like one account, biggest first.
    pub fn duplicate_groups(&mut self) -> Result<Vec<DuplicateGroup>> {
        let mut by_key: BTreeMap<(String, String), Vec<ItemId>> = BTreeMap::new();
        for s in self.session()?.items() {
            if s.kind == ItemKind::Login
                && crate::service::active(s)
                && let Some(key) = key_of(&s.overview.urls, s.overview.username.as_deref())
            {
                by_key.entry(key).or_default().push(s.id);
            }
        }
        let mut groups = Vec::new();
        for ((_, username), ids) in by_key.into_iter().filter(|(_, ids)| ids.len() > 1) {
            let mut items = Vec::new();
            for id in ids {
                items.push(self.session()?.item(id)?);
            }
            // Suggested one first: the richest, then the most recently changed.
            items.sort_by_key(|i| {
                (
                    std::cmp::Reverse(richness(i)),
                    std::cmp::Reverse(i.updated_at),
                    i.id,
                )
            });
            let mut numbers: HashMap<Option<String>, usize> = HashMap::new();
            let members: Vec<DuplicateMember> = items
                .iter()
                .map(|i| {
                    let password = i.details.password.as_ref().map(|p| p.expose().to_owned());
                    let next = numbers.len();
                    let password_group = *numbers.entry(password).or_insert(next);
                    DuplicateMember {
                        id: i.id.to_string(),
                        title: i.overview.title.clone(),
                        urls: i.overview.urls.clone(),
                        updated_at: i.updated_at.as_millis(),
                        password_group,
                        has_totp: i.details.totp.is_some(),
                        has_notes: i.details.notes.is_some(),
                        history: i.details.password_history.len(),
                    }
                })
                .collect();
            let site = items[0]
                .overview
                .urls
                .iter()
                .find_map(|u| Site::parse(u))
                .map(|s| s.registrable)
                .unwrap_or_default();
            groups.push(DuplicateGroup {
                site,
                username,
                same_password: numbers.len() == 1,
                members,
            });
        }
        groups.sort_by(|a, b| {
            b.members
                .len()
                .cmp(&a.members.len())
                .then_with(|| a.site.cmp(&b.site))
        });
        Ok(groups)
    }

    /// Merges `others` into `keep`: their websites, tags, favorite, one-time password and notes are
    /// added to it when it lacks them, and a different password is kept in its password history;
    /// then they move to Recently Deleted. Only logins of one group can be merged. Returns how many
    /// were merged.
    pub fn merge_duplicates(&mut self, keep: &str, others: &[String]) -> Result<usize> {
        let keep_id = parse_id::<ItemId>(keep)?;
        let mut kept = self.session()?.item(keep_id)?;
        let wanted = key_of(&kept.overview.urls, kept.overview.username.as_deref())
            .ok_or_else(|| AppError::Validation("the login to keep has no website".into()))?;
        let now = password365_model::Timestamp::now();
        let mut merged = Vec::new();
        for id in others {
            let other_id = parse_id::<ItemId>(id)?;
            if other_id == keep_id {
                continue;
            }
            let other = self.session()?.item(other_id)?;
            if other.kind != ItemKind::Login
                || key_of(&other.overview.urls, other.overview.username.as_deref()).as_ref()
                    != Some(&wanted)
            {
                return Err(AppError::Validation(
                    "these logins are not the same account".into(),
                ));
            }
            let mut overview = kept.overview.clone();
            for url in &other.overview.urls {
                if !overview.urls.contains(url) {
                    overview.urls.push(url.clone());
                }
            }
            for tag in &other.overview.tags {
                if !overview.tags.contains(tag) {
                    overview.tags.push(tag.clone());
                }
            }
            overview.favorite |= other.overview.favorite;
            kept.set_overview(overview, now);
            if kept.details.totp.is_none() {
                kept.details.totp.clone_from(&other.details.totp);
            }
            if kept.details.notes.is_none() {
                kept.details.notes.clone_from(&other.details.notes);
            }
            // A different password is not lost: it joins the history of the one that stays.
            if other.details.password != kept.details.password
                && let Some(password) = other.details.password.clone()
            {
                kept.details
                    .password_history
                    .record(password, other.updated_at);
            }
            merged.push(other_id);
        }
        if merged.is_empty() {
            return Ok(0);
        }
        kept.touch(now);
        self.session()?.save_item(&kept)?;
        for id in &merged {
            self.session()?.delete_item(*id)?;
        }
        Ok(merged.len())
    }
}

#[cfg(test)]
mod tests {
    use password365_crypto::secret::SecretString;

    use crate::item_input::ItemInput;
    use crate::service::tests::with_account;

    fn add(
        app: &mut crate::AppService,
        title: &str,
        url: &str,
        user: &str,
        pw: &str,
        notes: Option<&str>,
    ) -> String {
        app.create_item(&ItemInput {
            title: title.into(),
            urls: vec![url.into()],
            username: Some(user.into()),
            password: Some(SecretString::from(pw.to_owned())),
            notes: notes.map(|n| SecretString::from(n.to_owned())),
            ..ItemInput::default()
        })
        .unwrap()
        .summary
        .id
    }

    #[test]
    fn a_brands_country_sites_form_one_group_per_username_and_the_richest_is_suggested() {
        let mut fx = with_account();
        let es = add(
            &mut fx.app,
            "amazon.es",
            "https://www.amazon.es/ap/signin",
            "Ana",
            "pw1",
            None,
        );
        let br = add(
            &mut fx.app,
            "amazon.com.br",
            "https://www.amazon.com.br",
            "ana",
            "pw1",
            Some("note"),
        );
        let com = add(
            &mut fx.app,
            "amazon.com",
            "https://amazon.com",
            "ana",
            "other",
            None,
        );
        add(
            &mut fx.app,
            "amazon.com",
            "https://amazon.com",
            "bob",
            "x",
            None,
        );
        add(
            &mut fx.app,
            "GitHub",
            "https://github.com",
            "ana",
            "g",
            None,
        );
        let groups = fx.app.duplicate_groups().unwrap();
        assert_eq!(groups.len(), 1, "bob and GitHub are alone");
        let g = &groups[0];
        assert_eq!(
            (g.username.as_str(), g.members.len(), g.same_password),
            ("ana", 3, false)
        );
        assert_eq!(g.members[0].id, br, "the one with notes is suggested");
        let same: Vec<_> = g
            .members
            .iter()
            .filter(|m| m.password_group == g.members[0].password_group)
            .map(|m| m.id.clone())
            .collect();
        assert!(
            same.contains(&es) && !same.contains(&com),
            "two share a password, one differs"
        );
    }

    #[test]
    fn merging_adds_the_websites_keeps_a_different_password_in_history_and_deletes_the_rest() {
        let mut fx = with_account();
        let keep = add(
            &mut fx.app,
            "amazon.es",
            "https://amazon.es",
            "ana",
            "pw1",
            None,
        );
        let other = add(
            &mut fx.app,
            "amazon.com",
            "https://amazon.com",
            "ana",
            "old",
            Some("n"),
        );
        assert_eq!(
            fx.app
                .merge_duplicates(&keep, std::slice::from_ref(&other))
                .unwrap(),
            1
        );
        let item = fx.app.item(&keep).unwrap();
        assert!(item.summary.urls.contains(&"https://amazon.com".to_owned()));
        assert!(item.has_notes, "notes it lacked come along");
        assert_eq!(
            item.password_history_count, 1,
            "the other password is not lost"
        );
        assert_eq!(fx.app.deleted_items().unwrap().len(), 1);
        assert!(fx.app.duplicate_groups().unwrap().is_empty());
    }

    #[test]
    fn logins_of_different_accounts_are_refused() {
        let mut fx = with_account();
        let a = add(&mut fx.app, "amazon", "https://amazon.es", "ana", "1", None);
        let b = add(&mut fx.app, "amazon", "https://amazon.es", "bob", "2", None);
        let github = add(
            &mut fx.app,
            "GitHub",
            "https://github.com",
            "ana",
            "3",
            None,
        );
        assert!(
            fx.app
                .merge_duplicates(&a, std::slice::from_ref(&b))
                .is_err()
        );
        assert!(
            fx.app
                .merge_duplicates(&a, std::slice::from_ref(&github))
                .is_err()
        );
        assert!(
            fx.app.deleted_items().unwrap().is_empty(),
            "nothing was merged or deleted"
        );
    }
}
