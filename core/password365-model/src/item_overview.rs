//! Item overview: the fields needed for listing, searching and URL matching.
//!
//! Stored encrypted separately from [`crate::ItemDetails`] so secrets are only
//! decrypted on demand.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::error::required_text;

/// Maximum title length in characters.
pub const MAX_TITLE: usize = 256;

/// Non-secret descriptive fields of an item (still encrypted at rest).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemOverview {
    /// Display title, e.g. "GitHub".
    pub title: String,
    /// Username / e-mail, if any.
    pub username: Option<String>,
    /// Website URLs this item applies to.
    pub urls: Vec<String>,
    /// Free-form tags.
    pub tags: Vec<String>,
    /// Shown in Favorites.
    pub favorite: bool,
    /// In the Archive: kept, but out of the item list, search and filling.
    pub archived: bool,
}

impl ItemOverview {
    /// Creates an overview with a validated title.
    pub fn new(title: &str) -> Result<Self> {
        Ok(Self {
            title: required_text(title, "title", MAX_TITLE)?,
            ..Self::default()
        })
    }

    /// Sets the username (blank → none).
    #[must_use]
    pub fn with_username(mut self, username: &str) -> Self {
        self.username = non_blank(username);
        self
    }

    /// Adds a URL (blank and duplicate URLs are ignored).
    #[must_use]
    pub fn with_url(mut self, url: &str) -> Self {
        push_unique(&mut self.urls, url);
        self
    }

    /// Adds a tag (blank and duplicate tags are ignored).
    #[must_use]
    pub fn with_tag(mut self, tag: &str) -> Self {
        push_unique(&mut self.tags, tag);
        self
    }

    /// Case-insensitive match of `query` against title, username, URLs and tags.
    #[must_use]
    pub fn matches(&self, query: &str) -> bool {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }
        std::iter::once(self.title.as_str())
            .chain(self.username.as_deref())
            .chain(self.urls.iter().map(String::as_str))
            .chain(self.tags.iter().map(String::as_str))
            .any(|field| field.to_lowercase().contains(&needle))
    }
}

fn non_blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn push_unique(list: &mut Vec<String>, value: &str) {
    if let Some(v) = non_blank(value)
        && !list.contains(&v)
    {
        list.push(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn github() -> ItemOverview {
        ItemOverview::new("GitHub")
            .unwrap()
            .with_username("octo@example.com")
            .with_url("https://github.com")
            .with_tag("dev")
    }

    #[test]
    fn new_requires_title() {
        assert!(ItemOverview::new("  ").is_err());
        assert_eq!(ItemOverview::new(" X ").unwrap().title, "X");
    }

    #[test]
    fn builder_sets_fields() {
        let o = github();
        assert_eq!(o.username.as_deref(), Some("octo@example.com"));
        assert_eq!(o.urls, vec!["https://github.com"]);
        assert_eq!(o.tags, vec!["dev"]);
        assert!(!o.favorite);
    }

    #[test]
    fn blank_values_are_ignored() {
        let o = ItemOverview::new("t")
            .unwrap()
            .with_username("  ")
            .with_url("")
            .with_tag(" ");
        assert_eq!(o.username, None);
        assert!(o.urls.is_empty());
        assert!(o.tags.is_empty());
    }

    #[test]
    fn duplicates_are_ignored() {
        let o = ItemOverview::new("t")
            .unwrap()
            .with_url("a")
            .with_url(" a ")
            .with_tag("x")
            .with_tag("x");
        assert_eq!(o.urls.len(), 1);
        assert_eq!(o.tags.len(), 1);
    }

    #[test]
    fn matches_any_field_case_insensitively() {
        let o = github();
        assert!(o.matches("git"));
        assert!(o.matches("OCTO"));
        assert!(o.matches("github.com"));
        assert!(o.matches("DEV"));
        assert!(o.matches("  "));
        assert!(!o.matches("gitlab"));
    }
}
