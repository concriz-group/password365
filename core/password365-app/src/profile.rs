//! Profile (1Password's "Set your personal details / payment method"): the
//! identity and the card the browser extension fills into address and
//! payment forms. The choice is kept on this device next to the settings;
//! the values are read from the items only when the user asks to fill.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use password365_model::{Item, ItemKind};
use password365_vault::ItemSummary;
use serde::{Deserialize, Serialize};

use crate::browser::page_url_is_secure;
use crate::{AppError, AppService, Result};

const FILE: &str = "profile.json";

/// The chosen items (ids).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileChoice {
    /// Default identity.
    pub identity: Option<String>,
    /// Default payment card.
    pub card: Option<String>,
}

/// What a form needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    /// Name, address, phone, e-mail.
    Identity,
    /// Payment card.
    Card,
}

impl ProfileKind {
    const fn item_kind(self) -> ItemKind {
        match self {
            Self::Identity => ItemKind::Identity,
            Self::Card => ItemKind::CreditCard,
        }
    }
}

/// One chosen item as the Profile page and the browser menu show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileItem {
    /// Item id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Last four digits of a card number, if any.
    pub last4: Option<String>,
}

/// The identity and card in use (each may be unset).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProfileDto {
    /// Default identity.
    pub identity: Option<ProfileItem>,
    /// Default payment card.
    pub card: Option<ProfileItem>,
    /// Every identity, the default first (the browser menu offers the others too).
    pub identities: Vec<ProfileItem>,
    /// Every payment card, the default first.
    pub cards: Vec<ProfileItem>,
}

pub(crate) fn path(settings_path: Option<&Path>) -> Option<PathBuf> {
    settings_path.map(|p| p.with_file_name(FILE))
}

pub(crate) fn load(settings_path: Option<&Path>) -> ProfileChoice {
    path(settings_path)
        .and_then(|p| crate::json_file::read(&p))
        .unwrap_or_default()
}

/// Prefix of the keys that carry the item's own fields by label, so forms
/// asking for "Membership number" can be filled from a field of that name.
pub const LABEL_PREFIX: &str = "label:";

/// The values a form can take, keyed by what forms call them: HTML
/// autocomplete names (`given-name`, `postal-code`, `cc-number` …), tokens
/// of the item's country template (`cpf`, `house-number` …) and `label:<name>`
/// for the item's own fields.
#[must_use]
pub fn form_values(item: &Item, kind: ProfileKind) -> BTreeMap<String, String> {
    let fields = &item.details.custom_fields;
    let region = crate::field_layout::region_of(fields);
    let get = |label: &str| {
        fields
            .iter()
            .find(|f| f.label.eq_ignore_ascii_case(label))
            .map(|f| f.value.expose().trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    let template = password365_model::item_template::template_in(item.kind, &region);
    let mut values: BTreeMap<String, String> = template
        .iter()
        .filter_map(|spec| Some((spec.autofill.clone()?, get(&spec.label)?)))
        .collect();
    for f in fields {
        let own = !crate::field_layout::is_internal(&f.label)
            && !template
                .iter()
                .any(|s| s.label.eq_ignore_ascii_case(&f.label));
        if own && !f.value.is_empty() {
            values.insert(
                format!("{LABEL_PREFIX}{}", f.label.to_lowercase()),
                f.value.expose().trim().to_owned(),
            );
        }
    }
    match kind {
        ProfileKind::Identity => identity_extras(&mut values),
        ProfileKind::Card => {
            if let Some(expiry) = values.remove("cc-exp") {
                card_expiry(&mut values, &expiry);
            }
            if let Some(number) = values.get_mut("cc-number") {
                number.retain(|c| c.is_ascii_digit());
            }
        }
    }
    values
}

fn identity_extras(values: &mut BTreeMap<String, String>) {
    let full: Vec<&str> = ["given-name", "additional-name", "family-name"]
        .iter()
        .filter_map(|k| values.get(*k).map(String::as_str))
        .collect();
    if !full.is_empty() {
        let name = full.join(" ");
        values.insert("name".into(), name);
    }
    if let Some(address) = values.get("street-address").cloned() {
        let mut lines = address.lines().map(str::trim).filter(|l| !l.is_empty());
        if let Some(first) = lines.next() {
            values
                .entry("address-line1".into())
                .or_insert_with(|| first.to_owned());
        }
        let rest: Vec<&str> = lines.collect();
        if !rest.is_empty() {
            values
                .entry("address-line2".into())
                .or_insert_with(|| rest.join(", "));
        }
    } else if let Some(street) = values.get("address-line1").cloned() {
        // A form with one address box gets "street, number".
        let number = values
            .get("house-number")
            .map(|n| format!(", {n}"))
            .unwrap_or_default();
        values.insert("street-address".into(), format!("{street}{number}"));
    }
    if let Some(bday) = values.get("bday").cloned() {
        let parts: Vec<&str> = bday.split('-').collect();
        if let [year, month, day] = parts[..] {
            values.insert("bday-year".into(), year.to_owned());
            values.insert("bday-month".into(), month.to_owned());
            values.insert("bday-day".into(), day.to_owned());
        }
    }
}

/// `2029-04` → `cc-exp-month` 04, `cc-exp-year` 2029, `cc-exp` 04/29.
fn card_expiry(values: &mut BTreeMap<String, String>, expiry: &str) {
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    if let Some((year, month)) = expiry.split_once('-')
        && digits(year, 4)
        && digits(month, 2)
    {
        values.insert("cc-exp-month".into(), month.to_owned());
        values.insert("cc-exp-year".into(), year.to_owned());
        values.insert("cc-exp".into(), format!("{month}/{}", &year[2..]));
    }
}

#[cfg(test)]
mod expiry_tests {
    use super::*;

    #[test]
    fn odd_expiry_text_is_ignored_not_a_crash() {
        for odd in ["a€-04", "2029-4", "20x9-04", "-", ""] {
            let mut values = BTreeMap::new();
            card_expiry(&mut values, odd);
            assert!(values.is_empty(), "{odd}");
        }
        let mut values = BTreeMap::new();
        card_expiry(&mut values, "2029-04");
        assert_eq!(values["cc-exp"], "04/29");
    }
}

fn last4(item: &Item) -> Option<String> {
    let digits: String = form_values(item, ProfileKind::Card)
        .remove("cc-number")?
        .chars()
        .collect();
    (digits.len() >= 4).then(|| digits[digits.len() - 4..].to_owned())
}

impl AppService {
    /// The identity and card in use: the chosen ones, or the only one of
    /// their kind when nothing was chosen.
    pub fn profile(&mut self) -> Result<ProfileDto> {
        let identities = self.profile_items(ProfileKind::Identity)?;
        let cards = self.profile_items(ProfileKind::Card)?;
        let identity_id = self.profile_id(ProfileKind::Identity)?;
        let card_id = self.profile_id(ProfileKind::Card)?;
        let pick = |all: &[ProfileItem], id: Option<String>| {
            all.iter().find(|i| Some(&i.id) == id.as_ref()).cloned()
        };
        Ok(ProfileDto {
            identity: pick(&identities, identity_id),
            card: pick(&cards, card_id),
            identities,
            cards,
        })
    }

    /// Every usable item of `kind`, the default first, then by title.
    fn profile_items(&mut self, kind: ProfileKind) -> Result<Vec<ProfileItem>> {
        let default = self.profile_id(kind)?;
        let ids: Vec<String> = self
            .session()?
            .items()
            .into_iter()
            .filter(|s| s.kind == kind.item_kind() && crate::service::active(s))
            .map(|s| s.id.to_string())
            .collect();
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            let item = self.load_item(&id)?;
            items.push(ProfileItem {
                title: item.overview.title.clone(),
                last4: (kind == ProfileKind::Card).then(|| last4(&item)).flatten(),
                id,
            });
        }
        items.sort_by(|a, b| {
            (Some(&b.id) == default.as_ref())
                .cmp(&(Some(&a.id) == default.as_ref()))
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        Ok(items)
    }

    /// The item used for `kind`, if any.
    fn profile_id(&mut self, kind: ProfileKind) -> Result<Option<String>> {
        let chosen = match kind {
            ProfileKind::Identity => self.profile_choice.identity.clone(),
            ProfileKind::Card => self.profile_choice.card.clone(),
        };
        let session = self.session()?;
        let usable = |s: &&ItemSummary| s.kind == kind.item_kind() && crate::service::active(s);
        let candidates: Vec<String> = session
            .items()
            .into_iter()
            .filter(usable)
            .map(|s| s.id.to_string())
            .collect();
        Ok(match chosen {
            Some(id) if candidates.contains(&id) => Some(id),
            _ if candidates.len() == 1 => candidates.into_iter().next(),
            _ => None,
        })
    }

    /// Chooses (or clears, with `None`) the item used for `kind`.
    pub fn set_profile(&mut self, kind: ProfileKind, id: Option<&str>) -> Result<()> {
        if let Some(id) = id {
            let item = self.load_item(id)?;
            if item.kind != kind.item_kind() {
                return Err(AppError::Validation(format!(
                    "choose a {} item",
                    kind.item_kind().label()
                )));
            }
        }
        let mut choice = self.profile_choice.clone();
        let slot = match kind {
            ProfileKind::Identity => &mut choice.identity,
            ProfileKind::Card => &mut choice.card,
        };
        *slot = id.map(str::to_owned);
        if let Some(path) = path(self.config.settings_path.as_deref()) {
            crate::json_file::write(&path, &choice)?;
        }
        self.profile_choice = choice;
        Ok(())
    }

    /// The values to fill into a form on `page_url`. Card details only go
    /// to secure (https) pages.
    pub fn profile_form_values(
        &mut self,
        kind: ProfileKind,
        page_url: &str,
    ) -> Result<BTreeMap<String, String>> {
        self.profile_form_values_of(kind, None, page_url)
    }

    /// [`Self::profile_form_values`] of the item `item_id` (another identity
    /// or card than the default); `None` uses the default.
    pub fn profile_form_values_of(
        &mut self,
        kind: ProfileKind,
        item_id: Option<&str>,
        page_url: &str,
    ) -> Result<BTreeMap<String, String>> {
        if kind == ProfileKind::Card && !page_url_is_secure(page_url) {
            return Err(AppError::Validation(
                "card details are only filled on secure (https) pages".into(),
            ));
        }
        let id = match item_id {
            Some(id) => id.to_owned(),
            None => self
                .profile_id(kind)?
                .ok_or(AppError::NotFound("profile item"))?,
        };
        let item = self.load_item(&id)?;
        if item.kind != kind.item_kind() || item.overview.archived {
            return Err(AppError::NotFound("profile item"));
        }
        Ok(form_values(&item, kind))
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
