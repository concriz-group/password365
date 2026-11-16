//! Adding and editing items, categories and their country templates, the
//! generator, favorites, archive and delete.

use password365_app::AppService;
use password365_app::dto::{GenerateRequest, SecretField};
use password365_app::item_input::{FieldValue, ItemInput};
use password365_crypto::secret::SecretString;
use password365_model::ItemKind;
use password365_model::item_template::{FieldInput, FieldSpec, template_in};
use std::str::FromStr;

use crate::{Failure, Password365, Result};

/// `snake_case` name of a field input (`date`, `month_year`, …).
fn input_name(input: FieldInput) -> String {
    serde_json::to_value(input)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "text".into())
}

fn input_of(name: &str) -> Option<FieldInput> {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).ok()
}

fn invalid(message: impl Into<String>) -> Failure {
    Failure::Failed {
        code: "validation".into(),
        reason: message.into(),
    }
}

/// A field the category's template offers.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TemplateField {
    /// Label (also the translation key `fields.<label>`).
    pub label: String,
    /// Section heading ("" for the first block).
    pub section: String,
    /// `text`, `multiline`, `date`, `month_year`, `email`, `phone`, `url`, `totp`, …
    pub input: String,
    /// Hidden until revealed.
    pub concealed: bool,
}

impl From<&FieldSpec> for TemplateField {
    fn from(s: &FieldSpec) -> Self {
        Self {
            label: s.label.clone(),
            section: s.section.clone(),
            input: input_name(s.input),
            concealed: s.concealed,
        }
    }
}

/// A category (Login, Credit Card, Passport, …).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Category {
    /// Id (`login`, `card`, …; translation key `categories.<kind>`).
    pub kind: String,
    /// English name.
    pub label: String,
    /// Countries with fields of their own for it (`BR`, `PT`, …).
    pub regions: Vec<String>,
}

/// One field of an item being edited (values in clear while editing).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FieldDraft {
    /// Label.
    pub label: String,
    /// Value ("" leaves the field out).
    pub value: String,
    /// Section heading.
    pub section: String,
    /// Input kind (see [`TemplateField::input`]).
    pub input: String,
    /// Hidden until revealed.
    pub concealed: bool,
}

/// An item as the editor holds it. Secrets are in clear only while editing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ItemDraft {
    /// Category.
    pub kind: String,
    /// Title.
    pub title: String,
    /// Username ("" for none).
    pub username: String,
    /// Password ("" for none).
    pub password: String,
    /// One-time password setup key or `otpauth://` link ("" for none).
    pub totp: String,
    /// Notes ("" for none).
    pub notes: String,
    /// Websites.
    pub urls: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Starred.
    pub favorite: bool,
    /// Country whose template the fields follow ("" for the shared one).
    pub region: String,
    /// Other fields.
    pub fields: Vec<FieldDraft>,
    /// Version the draft was read at (0 for a new item): a change made
    /// meanwhile elsewhere (sync) makes the save fail instead of losing it.
    pub version: u64,
}

/// A generated password and how strong it is.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Generated {
    /// The password.
    pub value: String,
    /// `weak`, `fair`, `good`, `strong` or `excellent`.
    pub strength: String,
}

/// What the generator makes.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Recipe {
    /// Random characters.
    Password {
        /// Length.
        length: u32,
        /// Include 0–9.
        digits: bool,
        /// Include symbols.
        symbols: bool,
    },
    /// Random words.
    Passphrase {
        /// Number of words.
        words: u32,
    },
    /// Digits only.
    Pin {
        /// Number of digits.
        digits: u32,
    },
}

fn input_from(draft: &ItemDraft) -> Result<ItemInput> {
    let some = |s: &str| (!s.trim().is_empty()).then(|| SecretString::from(s));
    let fields = draft
        .fields
        .iter()
        .map(|f| {
            Ok(FieldValue {
                label: f.label.clone(),
                value: f.value.as_str().into(),
                section: Some(f.section.clone()),
                input: Some(
                    input_of(&f.input)
                        .ok_or_else(|| invalid(format!("unknown field input \"{}\"", f.input)))?,
                ),
                concealed: Some(f.concealed),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ItemInput {
        kind: Some(draft.kind.clone()),
        title: draft.title.clone(),
        username: (!draft.username.trim().is_empty()).then(|| draft.username.clone()),
        // An empty password / code / note clears it (the editor holds the current values).
        password: Some(some(&draft.password).unwrap_or_default()),
        totp: Some(some(&draft.totp).unwrap_or_default()),
        notes: Some(some(&draft.notes).unwrap_or_default()),
        urls: draft
            .urls
            .iter()
            .filter(|u| !u.trim().is_empty())
            .cloned()
            .collect(),
        tags: draft
            .tags
            .iter()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect(),
        favorite: draft.favorite,
        region: Some(draft.region.clone()),
        fields: Some(fields),
        ..ItemInput::default()
    })
}

fn reveal_all(app: &mut AppService, id: &str) -> password365_app::Result<ItemDraft> {
    let item = app.item(id)?;
    let secret = |app: &mut AppService, field| app.reveal(id, field).map(Option::unwrap_or_default);
    let mut fields = Vec::new();
    for f in item.fields {
        // Concealed values and one-time password secrets are not in the view:
        // read them, or saving the draft would erase them.
        let value = if f.concealed || f.value.is_none() {
            app.reveal_field(id, &f.label)?.unwrap_or_default()
        } else {
            f.value.unwrap_or_default()
        };
        fields.push(FieldDraft {
            label: f.label,
            value,
            section: f.section,
            input: input_name(f.input),
            concealed: f.concealed,
        });
    }
    Ok(ItemDraft {
        password: secret(app, SecretField::Password)?,
        totp: secret(app, SecretField::Totp)?,
        notes: secret(app, SecretField::Notes)?,
        kind: item.summary.kind,
        title: item.summary.title,
        username: item.summary.username.unwrap_or_default(),
        urls: item.summary.urls,
        tags: item.summary.tags,
        favorite: item.summary.favorite,
        region: item.region,
        fields,
        version: item.version,
    })
}

#[uniffi::export]
impl Password365 {
    /// Every category, in 1Password's order.
    pub fn categories(&self) -> Vec<Category> {
        AppService::categories()
            .into_iter()
            .map(|c| Category {
                kind: c.kind,
                label: c.label,
                regions: c.regions.into_keys().collect(),
            })
            .collect()
    }

    /// The fields a category offers in a country ("" for the shared template).
    pub fn template(&self, kind: String, region: String) -> Result<Vec<TemplateField>> {
        let kind = ItemKind::from_str(&kind).map_err(invalid)?;
        Ok(template_in(kind, &region)
            .iter()
            .map(TemplateField::from)
            .collect())
    }

    /// The country new items follow (from settings; "" for none).
    pub fn default_region(&self) -> String {
        self.app().settings().region
    }

    /// An empty draft of `kind`, with its template's fields for the default country.
    pub fn new_draft(&self, kind: String) -> Result<ItemDraft> {
        let region = self.default_region();
        let fields = self
            .template(kind.clone(), region.clone())?
            .into_iter()
            .map(|f| FieldDraft {
                label: f.label,
                value: String::new(),
                section: f.section,
                input: f.input,
                concealed: f.concealed,
            })
            .collect();
        Ok(ItemDraft {
            kind,
            title: String::new(),
            username: String::new(),
            password: String::new(),
            totp: String::new(),
            notes: String::new(),
            urls: Vec::new(),
            tags: Vec::new(),
            favorite: false,
            region,
            fields,
            version: 0,
        })
    }

    /// Item `id` with every value in clear, for the editor.
    pub fn edit_draft(&self, id: String) -> Result<ItemDraft> {
        Ok(reveal_all(&mut self.app(), &id)?)
    }

    /// Creates an item from a draft; returns its id.
    pub fn create_item(&self, draft: ItemDraft) -> Result<String> {
        let input = input_from(&draft)?;
        Ok(self.app().create_item(&input)?.summary.id)
    }

    /// Saves a draft over item `id` (fails if it changed since the draft was read).
    pub fn update_item(&self, id: String, draft: ItemDraft) -> Result<()> {
        let input = input_from(&draft)?;
        self.app().update_item(&id, draft.version, &input)?;
        Ok(())
    }

    /// Stars or unstars an item.
    pub fn set_favorite(&self, id: String, favorite: bool) -> Result<()> {
        Ok(self.app().set_favorite(&id, favorite)?)
    }

    /// Moves an item into (or out of) the Archive.
    pub fn set_archived(&self, id: String, archived: bool) -> Result<()> {
        Ok(self.app().set_archived(&id, archived)?)
    }

    /// Moves an item to Recently Deleted.
    pub fn delete_item(&self, id: String) -> Result<()> {
        Ok(self.app().delete_item(&id)?)
    }

    /// Archived items matching `query`.
    pub fn archived(&self, query: String) -> Result<Vec<crate::ItemRow>> {
        Ok(self
            .app()
            .archived_items(&query)?
            .into_iter()
            .map(crate::ItemRow::from)
            .collect())
    }

    /// Tags in use.
    pub fn tags(&self) -> Result<Vec<String>> {
        Ok(self.app().tags()?.into_iter().map(|t| t.name).collect())
    }

    /// A new password, passphrase or PIN (works while locked).
    pub fn generate(&self, recipe: Recipe) -> Result<Generated> {
        use password365_generator::charset::CharClass;
        use password365_generator::{PassphraseRecipe, PasswordRecipe};
        let request = match recipe {
            Recipe::Password {
                length,
                digits,
                symbols,
            } => {
                let mut classes = vec![CharClass::Lowercase, CharClass::Uppercase];
                if digits {
                    classes.push(CharClass::Digits);
                }
                if symbols {
                    classes.push(CharClass::Symbols);
                }
                GenerateRequest::Password(PasswordRecipe {
                    length: length as usize,
                    classes,
                    ..PasswordRecipe::default()
                })
            }
            Recipe::Passphrase { words } => GenerateRequest::Passphrase(PassphraseRecipe {
                words: words as usize,
                ..PassphraseRecipe::default()
            }),
            Recipe::Pin { digits } => GenerateRequest::Pin {
                digits: digits as usize,
            },
        };
        let out = self.app().generate(&request)?;
        Ok(Generated {
            value: out.value,
            strength: out.strength.to_owned(),
        })
    }
}
