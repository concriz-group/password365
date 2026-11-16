//! Item form data coming from a frontend, and how it maps onto the model.

use password365_crypto::secret::SecretString;
use password365_model::item_template::{FieldInput, spec_in};
use password365_model::{CustomField, Item, ItemDetails, ItemKind, ItemOverview, Timestamp};
use serde::Deserialize;

use crate::field_layout::{FieldMeta, default_meta, is_internal, layout_field, region_of};
use crate::{AppError, Result};

/// Item fields as entered in a form.
///
/// For updates, `None` secrets mean "keep the current value" and an empty
/// string means "remove it".
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ItemInput {
    /// Target vault id (create: default vault when `None`; update: keep vault when `None`).
    pub vault_id: Option<String>,
    /// `login`, `password` or `note` (create only; default `login`).
    pub kind: Option<String>,
    /// Title.
    pub title: String,
    /// Username.
    pub username: Option<String>,
    /// URLs.
    pub urls: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Favorite flag.
    pub favorite: bool,
    /// Password.
    pub password: Option<SecretString>,
    /// Notes.
    pub notes: Option<SecretString>,
    /// TOTP secret / `otpauth://` URI.
    pub totp: Option<SecretString>,
    /// The category's fields (and any extra ones), in order. `None` keeps
    /// the current ones; empty values are dropped.
    pub fields: Option<Vec<FieldValue>>,
    /// Country whose fields the item uses (`"BR"`; `""`: the shared ones).
    /// `None` keeps the item's current one.
    pub region: Option<String>,
    /// The item's own icon as a data URL; `""` removes it, `None` keeps it.
    pub icon: Option<String>,
}

/// One labelled value from the form.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FieldValue {
    /// Field label (the template's, or the user's own).
    pub label: String,
    /// Value.
    pub value: SecretString,
    /// Section heading (`None`: as the template says, or the first block).
    pub section: Option<String>,
    /// Kind of input (`None`: as the template says, or text).
    pub input: Option<FieldInput>,
    /// Hidden until revealed (`None`: as the template or the current field says).
    pub concealed: Option<bool>,
}

impl ItemInput {
    /// Validated overview built from the form.
    pub fn overview(&self) -> Result<ItemOverview> {
        let mut overview =
            ItemOverview::new(&self.title)?.with_username(self.username.as_deref().unwrap_or(""));
        for url in &self.urls {
            overview = overview.with_url(url);
        }
        for tag in &self.tags {
            overview = overview.with_tag(tag);
        }
        overview.favorite = self.favorite;
        if let Some(totp) = non_empty(self.totp.as_ref()) {
            password365_crypto::totp::Totp::parse(totp.expose())?;
        }
        for f in self.fields.iter().flatten() {
            if f.label
                .eq_ignore_ascii_case(crate::auto_type::SEQUENCE_LABEL)
            {
                crate::auto_type::validate(f.value.expose())?;
            }
        }
        Ok(overview)
    }

    /// Secret details for a new item of `kind`.
    pub fn new_details(&self, kind: ItemKind) -> Result<ItemDetails> {
        let mut custom_fields = self.custom_fields(kind, &[])?.unwrap_or_default();
        if let Some(icon) = self.icon.as_deref().filter(|i| !i.is_empty()) {
            crate::item_icon::set_icon(&mut custom_fields, Some(icon))?;
        }
        Ok(ItemDetails {
            password: non_empty(self.password.as_ref()),
            notes: non_empty(self.notes.as_ref()),
            totp: non_empty(self.totp.as_ref()),
            custom_fields,
            ..ItemDetails::default()
        })
    }

    /// The custom fields the form sets (`None` when the form leaves them),
    /// followed by the layout of any field not placed as the template says.
    fn custom_fields(
        &self,
        kind: ItemKind,
        current: &[CustomField],
    ) -> Result<Option<Vec<CustomField>>> {
        let Some(fields) = self.fields.as_ref() else {
            return Ok(None);
        };
        let region = match &self.region {
            Some(r)
                if r.is_empty() || (r.len() == 2 && r.chars().all(|c| c.is_ascii_uppercase())) =>
            {
                r.clone()
            }
            Some(r) => return Err(AppError::Validation(format!("unknown country \"{r}\""))),
            None => region_of(current),
        };
        let mut out: Vec<CustomField> = Vec::new();
        let mut metas: Vec<(String, FieldMeta)> = Vec::new();
        for f in fields
            .iter()
            .filter(|f| !f.value.is_empty() && !f.label.trim().is_empty())
        {
            let label = f.label.trim();
            if is_internal(label) || out.iter().any(|o| o.label.eq_ignore_ascii_case(label)) {
                return Err(AppError::Validation(format!(
                    "There is already a field called \"{label}\"."
                )));
            }
            let fallback = default_meta(kind, &region, label);
            let meta = FieldMeta {
                section: f
                    .section
                    .as_ref()
                    .map_or(fallback.section, |s| s.trim().to_owned()),
                input: f.input.unwrap_or(fallback.input),
            };
            if meta.input == FieldInput::Totp {
                password365_crypto::totp::Totp::parse(f.value.expose())?;
            }
            if meta.input == FieldInput::SignInWith {
                crate::service::parse_id::<password365_model::ItemId>(f.value.expose())?;
            }
            out.push(CustomField {
                label: label.to_owned(),
                value: f.value.clone(),
                concealed: f.concealed.unwrap_or_else(|| {
                    spec_in(kind, &region, label).map_or_else(
                        || current.iter().any(|c| c.label == label && c.concealed),
                        |s| s.concealed,
                    )
                }),
            });
            metas.push((label.to_owned(), meta));
        }
        let borrowed: Vec<(&str, FieldMeta)> =
            metas.iter().map(|(l, m)| (l.as_str(), m.clone())).collect();
        out.extend(layout_field(kind, &region, &borrowed));
        Ok(Some(out))
    }

    /// Applies the form to an existing item (bumping its version).
    pub fn apply_to(&self, item: &mut Item, now: Timestamp) -> Result<()> {
        let mut overview = self.overview()?;
        // The form never moves an item in or out of the Archive.
        overview.archived = item.overview.archived;
        item.set_overview(overview, now);
        if let Some(password) = &self.password {
            item.set_password(Some(password.clone()), now);
        }
        if let Some(notes) = &self.notes {
            item.details.notes = non_empty(Some(notes));
        }
        if let Some(totp) = &self.totp {
            item.details.totp = non_empty(Some(totp));
        }
        let icon = match self.icon.as_deref() {
            None => crate::item_icon::icon_of(&item.details.custom_fields).map(str::to_owned),
            Some("") => None,
            Some(icon) => Some(icon.to_owned()),
        };
        if let Some(mut fields) = self.custom_fields(item.kind, &item.details.custom_fields)? {
            crate::passkeys::keep_passkey(&item.details.custom_fields, &mut fields)?;
            item.details.custom_fields = fields;
        }
        crate::item_icon::set_icon(&mut item.details.custom_fields, icon.as_deref())?;
        Ok(())
    }
}

fn non_empty(value: Option<&SecretString>) -> Option<SecretString> {
    value.filter(|v| !v.is_empty()).cloned()
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemKind, VaultId};

    use super::*;

    fn input() -> ItemInput {
        ItemInput {
            title: " GitHub ".into(),
            username: Some("octo".into()),
            urls: vec!["https://github.com".into()],
            tags: vec!["dev".into()],
            favorite: true,
            password: Some("pw".into()),
            notes: Some("".into()),
            totp: Some("JBSWY3DPEHPK3PXP".into()),
            ..ItemInput::default()
        }
    }

    #[test]
    fn builds_overview_and_details() {
        let i = input();
        let o = i.overview().unwrap();
        assert_eq!(
            (o.title.as_str(), o.username.as_deref(), o.favorite),
            ("GitHub", Some("octo"), true)
        );
        assert_eq!((o.urls.len(), o.tags.len()), (1, 1));
        let d = i.new_details(ItemKind::Login).unwrap();
        assert_eq!(d.password.unwrap().expose(), "pw");
        assert!(d.notes.is_none(), "empty notes dropped");
        assert_eq!(d.totp.unwrap().expose(), "JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn invalid_title_is_rejected() {
        let i = ItemInput {
            title: " ".into(),
            ..ItemInput::default()
        };
        assert_eq!(i.overview().unwrap_err().code(), "validation");
    }

    #[test]
    fn deserializes_with_defaults() {
        let i: ItemInput = serde_json::from_str(r#"{"title":"x","password":"p"}"#).unwrap();
        assert_eq!(i.title, "x");
        assert!(i.urls.is_empty() && !i.favorite && i.notes.is_none());
    }

    fn existing() -> Item {
        Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("Old").unwrap(),
            ItemDetails::with_password("old".into()).notes("keep me".into()),
            Timestamp::from_millis(1),
        )
    }

    #[test]
    fn update_keeps_unspecified_secrets() {
        let mut item = existing();
        let form = ItemInput {
            title: "New".into(),
            ..ItemInput::default()
        };
        form.apply_to(&mut item, Timestamp::from_millis(2)).unwrap();
        assert_eq!(item.overview.title, "New");
        assert_eq!(item.details.password.as_ref().unwrap().expose(), "old");
        assert_eq!(item.details.notes.as_ref().unwrap().expose(), "keep me");
        assert!(item.version > 1);
    }

    #[test]
    fn update_changes_and_clears_secrets() {
        let mut item = existing();
        let form = ItemInput {
            title: "Old".into(),
            password: Some("new".into()),
            notes: Some("".into()),
            totp: Some("JBSWY3DPEHPK3PXP".into()),
            ..ItemInput::default()
        };
        form.apply_to(&mut item, Timestamp::from_millis(2)).unwrap();
        assert_eq!(item.details.password.as_ref().unwrap().expose(), "new");
        assert_eq!(item.details.password_history.len(), 1);
        assert!(item.details.notes.is_none());
        assert_eq!(
            item.details.totp.as_ref().unwrap().expose(),
            "JBSWY3DPEHPK3PXP"
        );
    }

    #[test]
    fn update_with_invalid_title_changes_nothing() {
        let mut item = existing();
        let form = ItemInput {
            title: String::new(),
            password: Some("x".into()),
            ..ItemInput::default()
        };
        assert!(form.apply_to(&mut item, Timestamp::from_millis(2)).is_err());
        assert_eq!(item.version, 1);
    }
}
