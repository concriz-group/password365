//! A login read from an external file, before it becomes a vault item.

use password365_crypto::secret::SecretString;
use password365_model::{Item, ItemDetails, ItemKind, ItemOverview, Result, Timestamp, VaultId};

use crate::url_key;

/// Title used when neither a name nor a URL host is available.
pub const FALLBACK_TITLE: &str = "Imported login";

/// A login parsed from an import file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedLogin {
    /// Title (may be empty in the source; see [`ImportedLogin::effective_title`]).
    pub title: String,
    /// Website URL.
    pub url: Option<String>,
    /// Username.
    pub username: Option<String>,
    /// Password.
    pub password: Option<SecretString>,
    /// Notes.
    pub notes: Option<SecretString>,
    /// TOTP secret / URI.
    pub totp: Option<SecretString>,
    /// Tags (from the source's folders, groups, vaults or tags).
    pub tags: Vec<String>,
    /// Marked as a favorite in the source.
    pub favorite: bool,
    /// A row with only a title and notes: imported as a secure note.
    pub note_only: bool,
}

impl ImportedLogin {
    /// Title, falling back to the URL host, then to [`FALLBACK_TITLE`].
    #[must_use]
    pub fn effective_title(&self) -> String {
        let title = self.title.trim();
        if !title.is_empty() {
            return title.to_owned();
        }
        self.url
            .as_deref()
            .and_then(url_key::display_host)
            .unwrap_or_else(|| FALLBACK_TITLE.to_owned())
    }

    /// Converts into a new Login (or, for a notes-only row, Secure Note) item in `vault_id`.
    pub fn into_item(self, vault_id: VaultId, now: Timestamp) -> Result<Item> {
        let mut overview = ItemOverview::new(&self.effective_title())?;
        for tag in &self.tags {
            overview = overview.with_tag(tag);
        }
        overview.favorite = self.favorite;
        if let Some(username) = &self.username {
            overview = overview.with_username(username);
        }
        if let Some(url) = &self.url {
            overview = overview.with_url(url);
        }
        let details = ItemDetails {
            password: self.password,
            notes: self.notes,
            totp: self.totp,
            ..ItemDetails::default()
        };
        let kind = if self.note_only {
            ItemKind::SecureNote
        } else {
            ItemKind::Login
        };
        Ok(Item::new(vault_id, kind, overview, details, now))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn login(
        title: &str,
        url: Option<&str>,
        user: Option<&str>,
        pw: Option<&str>,
    ) -> ImportedLogin {
        ImportedLogin {
            title: title.into(),
            url: url.map(Into::into),
            username: user.map(Into::into),
            password: pw.map(Into::into),
            notes: None,
            totp: None,
            tags: Vec::new(),
            favorite: false,
            note_only: false,
        }
    }

    #[test]
    fn title_fallbacks() {
        assert_eq!(
            login("GitHub", None, None, None).effective_title(),
            "GitHub"
        );
        assert_eq!(
            login(" ", Some("https://www.github.com/x"), None, None).effective_title(),
            "github.com"
        );
        assert_eq!(
            login("", Some("nonsense"), None, None).effective_title(),
            FALLBACK_TITLE
        );
        assert_eq!(
            login("", None, None, None).effective_title(),
            FALLBACK_TITLE
        );
    }

    #[test]
    fn converts_to_login_item() {
        let mut imported = login("Mail", Some("https://mail.com"), Some("me"), Some("pw"));
        imported.notes = Some("n".into());
        imported.totp = Some("otpauth://x".into());
        let vault = VaultId::new();
        let item = imported
            .into_item(vault, Timestamp::from_millis(1))
            .unwrap();
        assert_eq!(item.vault_id, vault);
        assert_eq!(item.kind, ItemKind::Login);
        assert_eq!(item.overview.title, "Mail");
        assert_eq!(item.overview.username.as_deref(), Some("me"));
        assert_eq!(item.overview.urls, vec!["https://mail.com"]);
        assert_eq!(item.details.password.unwrap().expose(), "pw");
        assert_eq!(item.details.notes.unwrap().expose(), "n");
        assert_eq!(item.details.totp.unwrap().expose(), "otpauth://x");
    }

    #[test]
    fn keeps_tags_and_favorite_and_makes_notes() {
        let mut imported = login("Wi-Fi", None, None, None);
        imported.tags = vec!["Home".into()];
        imported.favorite = true;
        imported.note_only = true;
        let item = imported
            .into_item(VaultId::new(), Timestamp::from_millis(1))
            .unwrap();
        assert_eq!(item.kind, ItemKind::SecureNote);
        assert_eq!(item.overview.tags, vec!["Home"]);
        assert!(item.overview.favorite);
    }
}
