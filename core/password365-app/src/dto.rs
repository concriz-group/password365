//! Serializable data shapes exchanged with frontends.
//!
//! None of the outgoing DTOs carries a secret: secrets are fetched one field at
//! a time through `AppService::reveal`.

use password365_generator::{PassphraseRecipe, PasswordRecipe};
use password365_model::{Item, Vault};
use password365_vault::ItemSummary;
use serde::{Deserialize, Serialize};

/// Lock state of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AppStatus {
    /// An account (store) exists.
    pub initialized: bool,
    /// The vault is currently unlocked.
    pub unlocked: bool,
}

/// A vault in listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VaultDto {
    /// Vault id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Number of items.
    pub item_count: usize,
}

impl VaultDto {
    /// Builds the DTO.
    #[must_use]
    pub fn new(vault: &Vault, item_count: usize) -> Self {
        Self {
            id: vault.id.to_string(),
            name: vault.name.clone(),
            item_count,
        }
    }
}

/// An item in listings (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemSummaryDto {
    /// Item id.
    pub id: String,
    /// Owning vault id.
    pub vault_id: String,
    /// `login`, `password` or `note`.
    pub kind: String,
    /// Title.
    pub title: String,
    /// Username.
    pub username: Option<String>,
    /// URLs.
    pub urls: Vec<String>,
    /// Favorite flag.
    pub favorite: bool,
    /// Tags.
    pub tags: Vec<String>,
    /// In the Archive.
    pub archived: bool,
    /// Last change (ms since epoch).
    pub updated_at: i64,
    /// Two different passwords were set at once on two devices: the newer is
    /// current, the other is in the password history. Cleared by the user.
    pub needs_review: bool,
}

impl From<&ItemSummary> for ItemSummaryDto {
    fn from(s: &ItemSummary) -> Self {
        Self {
            id: s.id.to_string(),
            vault_id: s.vault_id.to_string(),
            kind: s.kind.to_string(),
            title: s.overview.title.clone(),
            username: s.overview.username.clone(),
            urls: s.overview.urls.clone(),
            favorite: s.overview.favorite,
            tags: s.overview.tags.clone(),
            archived: s.overview.archived,
            updated_at: s.updated_at.as_millis(),
            needs_review: false,
        }
    }
}

/// Full item view (secrets replaced by presence flags).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemDto {
    /// Listing fields.
    #[serde(flatten)]
    pub summary: ItemSummaryDto,
    /// A password is stored.
    pub has_password: bool,
    /// A TOTP secret is stored.
    pub has_totp: bool,
    /// Notes are stored.
    pub has_notes: bool,
    /// Number of previous passwords kept.
    pub password_history_count: usize,
    /// The category's fields and any extra ones, in order. Concealed values
    /// are not included (reveal them one at a time).
    pub fields: Vec<FieldDto>,
    /// Country whose fields the item uses (`""`: the shared ones).
    pub region: String,
    /// The item's own icon (a data URL), if it has one.
    pub icon: Option<String>,
    /// Attached files.
    pub attachments: Vec<crate::attachments::AttachmentDto>,
    /// The login's passkey (site and account; never the key).
    pub passkey: Option<crate::passkeys::PasskeyDto>,
    /// Creation time (ms since epoch).
    pub created_at: i64,
    /// Version counter (send it back when saving to detect conflicts).
    pub version: u64,
}

impl From<&Item> for ItemDto {
    fn from(item: &Item) -> Self {
        Self {
            summary: ItemSummaryDto::from(&ItemSummary::from(item)),
            has_password: item.details.password.is_some(),
            has_totp: item.details.totp.is_some(),
            has_notes: item.details.notes.is_some(),
            password_history_count: item.details.password_history.len(),
            fields: crate::field_layout::describe(item.kind, &item.details.custom_fields)
                .into_iter()
                .map(|(f, meta)| FieldDto::new(f, meta))
                .collect(),
            region: crate::field_layout::region_of(&item.details.custom_fields),
            icon: crate::item_icon::icon_of(&item.details.custom_fields).map(str::to_owned),
            passkey: crate::passkeys::dto_of(&item.details.custom_fields),
            attachments: item
                .details
                .attachments
                .iter()
                .map(|a| crate::attachments::AttachmentDto::new(a, true))
                .collect(),
            created_at: item.created_at.as_millis(),
            version: item.version,
        }
    }
}

/// Which secret to reveal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretField {
    /// The password.
    Password,
    /// Notes.
    Notes,
    /// TOTP secret.
    Totp,
}

/// What to generate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerateRequest {
    /// Random characters.
    Password(PasswordRecipe),
    /// Words from the EFF list.
    Passphrase(PassphraseRecipe),
    /// Digits only.
    Pin {
        /// Number of digits.
        digits: usize,
    },
}

/// A generated secret (returned to the UI on purpose — the user asked for it).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeneratedDto {
    /// The secret.
    pub value: String,
    /// Estimated entropy in bits.
    pub bits: f64,
    /// Human label (weak … excellent).
    pub strength: &'static str,
}

/// Outcome of a CSV import.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportSummary {
    /// New items created.
    pub imported: usize,
    /// Already present with the same password.
    pub identical: usize,
    /// Present with a different password (not updated).
    pub changed: usize,
    /// Passwords updated (when requested).
    pub updated: usize,
    /// Repeated rows in the file.
    pub duplicates_in_file: usize,
    /// Skipped rows as `(line, reason)`.
    pub skipped: Vec<(u64, String)>,
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview, Timestamp, VaultId};

    use super::*;

    fn item() -> Item {
        let mut details = ItemDetails::with_password("TOPSECRET".into()).notes("NOTESECRET".into());
        details.change_password(Some("NEWSECRET".into()), Timestamp::from_millis(2));
        Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("Mail")
                .unwrap()
                .with_username("me")
                .with_tag("t"),
            details,
            Timestamp::from_millis(1),
        )
    }

    #[test]
    fn item_dto_has_flags_but_no_secrets() {
        let dto = ItemDto::from(&item());
        let json = serde_json::to_string(&dto).unwrap();
        assert!(!json.contains("SECRET"), "{json}");
        assert!(dto.has_password && dto.has_notes && !dto.has_totp);
        assert_eq!(dto.password_history_count, 1);
        assert_eq!(dto.summary.kind, "login");
        assert!(json.contains("\"title\":\"Mail\""), "summary is flattened");
    }

    #[test]
    fn vault_dto() {
        let v = Vault::new("Work", Timestamp::from_millis(1)).unwrap();
        let dto = VaultDto::new(&v, 3);
        assert_eq!((dto.name.as_str(), dto.item_count), ("Work", 3));
        assert_eq!(dto.id, v.id.to_string());
    }

    #[test]
    fn generate_request_json_shape() {
        let req: GenerateRequest = serde_json::from_str(r#"{"type":"pin","digits":6}"#).unwrap();
        assert_eq!(req, GenerateRequest::Pin { digits: 6 });
        let json =
            serde_json::to_string(&GenerateRequest::Password(PasswordRecipe::default())).unwrap();
        assert!(
            json.starts_with(r#"{"type":"password","length":20"#),
            "{json}"
        );
    }

    #[test]
    fn secret_field_json() {
        assert_eq!(
            serde_json::to_string(&SecretField::Totp).unwrap(),
            "\"totp\""
        );
    }
}

/// A one-time password as the item detail shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TotpCodeDto {
    /// The code (e.g. `"042137"`).
    pub code: String,
    /// Seconds until it changes.
    pub remaining: u64,
    /// Seconds each code lasts.
    pub period: u64,
}

impl From<password365_crypto::totp::Code> for TotpCodeDto {
    fn from(c: password365_crypto::totp::Code) -> Self {
        Self {
            code: c.code,
            remaining: c.remaining,
            period: c.period,
        }
    }
}

/// "Your master password was changed on Mac": shown until this device uses it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PasswordUpdateDto {
    /// Where it was changed.
    pub device_name: String,
    /// When (ms since epoch).
    pub changed_at: i64,
}

/// A tag and how many items carry it (sidebar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagDto {
    /// Tag text.
    pub name: String,
    /// Items with this tag.
    pub count: usize,
}

impl TagDto {
    /// Every tag used by `summaries`, counted, in alphabetical order
    /// (case-insensitive).
    #[must_use]
    pub fn collect<'a>(summaries: impl IntoIterator<Item = &'a ItemSummary>) -> Vec<Self> {
        let mut counts = std::collections::BTreeMap::<(String, String), usize>::new();
        for tag in summaries.into_iter().flat_map(|s| &s.overview.tags) {
            *counts.entry((tag.to_lowercase(), tag.clone())).or_default() += 1;
        }
        counts
            .into_iter()
            .map(|((_, name), count)| Self { name, count })
            .collect()
    }
}

/// A custom or category field as the item view shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldDto {
    /// Label.
    pub label: String,
    /// Hidden until revealed.
    pub concealed: bool,
    /// The value, only when not concealed.
    pub value: Option<String>,
    /// Section heading (`""`: the item's first block).
    pub section: String,
    /// Kind of input.
    pub input: password365_model::item_template::FieldInput,
}

impl FieldDto {
    /// The view of `f`, placed as `meta` says. A one-time password's secret
    /// is never sent: the view asks for the current code instead.
    #[must_use]
    pub fn new(f: &password365_model::CustomField, meta: crate::field_layout::FieldMeta) -> Self {
        let hidden =
            f.concealed || meta.input == password365_model::item_template::FieldInput::Totp;
        Self {
            label: f.label.clone(),
            concealed: f.concealed,
            value: (!hidden).then(|| f.value.expose().to_owned()),
            section: meta.section,
            input: meta.input,
        }
    }
}

/// A category and its fields (the "What would you like to add?" screen and
/// the editor).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CategoryDto {
    /// Stable id (`login`, `card`, ...).
    pub kind: String,
    /// Display name.
    pub label: String,
    /// Plural display name (category filter).
    pub plural: String,
    /// Extra fields (empty for logins, passwords and secure notes).
    pub fields: Vec<password365_model::item_template::FieldSpec>,
    /// Countries with fields of their own for this category (`"BR"` → fields).
    pub regions:
        std::collections::BTreeMap<String, Vec<password365_model::item_template::FieldSpec>>,
}

impl CategoryDto {
    /// Every category, in 1Password's order.
    #[must_use]
    pub fn all() -> Vec<Self> {
        password365_model::ItemKind::ALL
            .iter()
            .map(|&k| Self {
                kind: k.as_str().to_owned(),
                label: k.label().to_owned(),
                plural: k.plural().to_owned(),
                fields: password365_model::item_template::template(k).to_vec(),
                regions: password365_model::item_template::region_templates(k)
                    .into_iter()
                    .map(|(code, fields)| (code.to_owned(), fields.to_vec()))
                    .collect(),
            })
            .collect()
    }
}
