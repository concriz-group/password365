//! An item read from another password manager's export, in any category,
//! before it becomes a vault item.

use password365_model::ItemKind;

/// One field of an imported item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedField {
    /// Section heading ("" for the first block).
    pub section: String,
    /// Label (lower case, like the templates').
    pub label: String,
    /// Value as Password365 stores it (dates `YYYY-MM-DD`, months `YYYY-MM`).
    pub value: String,
    /// Hidden until revealed.
    pub concealed: bool,
    /// `text`, `date`, `month_year`, `email`, `phone`, `url`, `multiline` or `totp`.
    pub input: &'static str,
}

/// An item from a `.1pux` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedItem {
    /// Category.
    pub kind: ItemKind,
    /// Title.
    pub title: String,
    /// Username (logins).
    pub username: Option<String>,
    /// Websites.
    pub urls: Vec<String>,
    /// Password (logins, passwords).
    pub password: Option<String>,
    /// Notes.
    pub notes: Option<String>,
    /// One-time password secret / URI.
    pub totp: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Favorite.
    pub favorite: bool,
    /// In the Archive.
    pub archived: bool,
    /// Other fields.
    pub fields: Vec<ImportedField>,
}
