//! Operations used by the browser extension: find logins for a page, release
//! credentials for filling, and decide whether a submitted login should be
//! saved or updated.
//!
//! Every decision is made here, in the trusted process — the extension only
//! ever receives credentials whose saved website matches the page.

use password365_crypto::secret::SecretString;
use password365_import::url_key::display_host;
use password365_match::{MatchLevel, Site, fill_allowed, match_level};
use password365_model::{Item, ItemDetails, ItemId, ItemKind, ItemOverview, Timestamp};
use password365_vault::ItemSummary;
use serde::{Deserialize, Serialize};

use crate::dto::ItemDto;
use crate::{AppError, AppService, Result};

/// A login offered for the current page (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoginCandidate {
    /// Item id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Username, if any.
    pub username: Option<String>,
    /// `origin`, `host`, `site` or `related` (another domain of the same account).
    pub match_level: &'static str,
    /// A related-domain match on a site this login was never used on: filling needs a confirmation.
    pub new_site: bool,
}

/// Credentials released for one fill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FillCredentials {
    /// Username.
    pub username: Option<String>,
    /// Password.
    pub password: Option<String>,
}

/// A login form the user just submitted.
#[derive(Debug, Clone, Deserialize)]
pub struct SubmittedLogin {
    /// URL of the page the form was on.
    pub page_url: String,
    /// Username / e-mail field value.
    #[serde(default)]
    pub username: Option<String>,
    /// Password field value.
    pub password: SecretString,
    /// Page title, used as item title for new logins.
    #[serde(default)]
    pub title: Option<String>,
}

/// What to propose to the user after a submit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SubmissionCheck {
    /// Not in the vault → offer "Save".
    New,
    /// Already saved with this password → nothing to do.
    Known {
        /// Item id.
        id: String,
    },
    /// Saved with a different password → offer "Update".
    PasswordChanged {
        /// Item id.
        id: String,
    },
}

impl AppService {
    /// Logins whose saved website matches `page_url`, strongest match first.
    pub fn logins_for_url(&mut self, page_url: &str) -> Result<Vec<LoginCandidate>> {
        let page = page_site(page_url)?;
        let session = self.session()?;
        let seen = crate::seen_sites::read(session);
        let mut found: Vec<(MatchLevel, &ItemSummary)> = session
            .items()
            .into_iter()
            .filter(|s| is_fillable_kind(s.kind) && crate::service::active(s))
            .filter_map(|s| best_fill_match(&page, &s.overview.urls).map(|level| (level, s)))
            .collect();
        found.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.overview.title.cmp(&b.1.overview.title))
        });
        Ok(found
            .into_iter()
            .map(|(level, s)| LoginCandidate {
                id: s.id.to_string(),
                title: s.overview.title.clone(),
                username: s.overview.username.clone(),
                match_level: level_name(level),
                new_site: level == MatchLevel::Related
                    && !crate::seen_sites::seen(&seen, &s.id.to_string(), &page.registrable),
            })
            .collect())
    }

    /// Releases username + password of `id`, but only for a matching page. A login that matches only
    /// through a related domain, on a site it was never used on, is released only with `confirmed`
    /// (the person agreed to this site); the site is then remembered.
    pub fn fill_credentials(
        &mut self,
        id: &str,
        page_url: &str,
        confirmed: bool,
    ) -> Result<FillCredentials> {
        let page = page_site(page_url)?;
        let item = self.load_item(id)?;
        let Some(level) = best_fill_match(&page, &item.overview.urls) else {
            return Err(AppError::Validation(
                "this login is not for this website".into(),
            ));
        };
        if level == MatchLevel::Related {
            let session = self.session()?;
            let known =
                crate::seen_sites::seen(&crate::seen_sites::read(session), id, &page.registrable);
            if !known && !confirmed {
                return Err(AppError::NewSite);
            }
            if !known {
                crate::seen_sites::remember(session, id, &page.registrable)?;
            }
        }
        Ok(FillCredentials {
            username: item.overview.username.clone(),
            password: item
                .details
                .password
                .as_ref()
                .map(|p| p.expose().to_owned()),
        })
    }

    /// Classifies a submitted login: new, already known, or changed password.
    pub fn check_submitted(&mut self, login: &SubmittedLogin) -> Result<SubmissionCheck> {
        Ok(match self.existing_login(login)? {
            None => SubmissionCheck::New,
            Some(item) if item.details.password.as_ref() == Some(&login.password) => {
                SubmissionCheck::Known {
                    id: item.id.to_string(),
                }
            }
            Some(item) => SubmissionCheck::PasswordChanged {
                id: item.id.to_string(),
            },
        })
    }

    /// Saves a new login or updates the password of the existing one.
    pub fn save_submitted(
        &mut self,
        login: &SubmittedLogin,
        vault_id: Option<&str>,
    ) -> Result<ItemDto> {
        if login.password.is_empty() {
            return Err(AppError::Validation(
                "nothing to save: the password is empty".into(),
            ));
        }
        if let Some(mut item) = self.existing_login(login)? {
            if item.set_password(Some(login.password.clone()), Timestamp::now()) {
                self.session()?.save_item(&item)?;
            }
            return Ok(ItemDto::from(&item));
        }
        let page = page_site(&login.page_url)?;
        let title = login
            .title
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .map_or_else(
                || display_host(&page.origin()).unwrap_or_else(|| page.host.clone()),
                str::to_owned,
            );
        let overview = ItemOverview::new(&title)?
            .with_username(login.username.as_deref().unwrap_or(""))
            .with_url(&page.origin());
        let vault = self.vault_or_default(vault_id)?;
        let id = self.session()?.add_item(
            vault,
            ItemKind::Login,
            overview,
            ItemDetails::with_password(login.password.clone()),
        )?;
        self.item(&id.to_string())
    }

    /// The saved login for the same site and username, if any.
    fn existing_login(&mut self, login: &SubmittedLogin) -> Result<Option<Item>> {
        let page = page_site(&login.page_url)?;
        let username = normalize_username(login.username.as_deref());
        let session = self.session()?;
        let id: Option<ItemId> = session
            .items()
            .into_iter()
            .filter(|s| is_fillable_kind(s.kind) && crate::service::active(s))
            .filter(|s| normalize_username(s.overview.username.as_deref()) == username)
            .find(|s| best_fill_match(&page, &s.overview.urls).is_some())
            .map(|s| s.id);
        id.map(|id| session.item(id))
            .transpose()
            .map_err(Into::into)
    }
}

/// An `https` page, or a page on this computer (development servers).
pub(crate) fn page_url_is_secure(page_url: &str) -> bool {
    Site::parse(page_url).is_some_and(|site| {
        site.scheme == "https"
            || matches!(
                site.host.as_str(),
                "localhost" | "127.0.0.1" | "[::1]" | "::1"
            )
    })
}

fn page_site(page_url: &str) -> Result<Site> {
    Site::parse(page_url).ok_or_else(|| AppError::Validation("unsupported page address".into()))
}

/// Best match among an item's URLs for which filling is allowed.
fn best_fill_match(page: &Site, urls: &[String]) -> Option<MatchLevel> {
    urls.iter()
        .filter_map(|u| Site::parse(u))
        .filter(|saved| fill_allowed(page, saved))
        .filter_map(|saved| match_level(page, &saved))
        .min()
}

fn is_fillable_kind(kind: ItemKind) -> bool {
    matches!(kind, ItemKind::Login | ItemKind::Password)
}

fn normalize_username(username: Option<&str>) -> String {
    username.unwrap_or("").trim().to_lowercase()
}

const fn level_name(level: MatchLevel) -> &'static str {
    match level {
        MatchLevel::Origin => "origin",
        MatchLevel::Host => "host",
        MatchLevel::Site => "site",
        MatchLevel::Related => "related",
    }
}

#[cfg(test)]
#[path = "browser_tests.rs"]
mod tests;
