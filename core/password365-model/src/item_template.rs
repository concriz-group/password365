//! The fields each category shows (1Password's templates), read from the
//! repository's `templates/` folder: `categories.json` for every country and
//! `regions/<CODE>.json` for a country's own fields (documents, address
//! parts, masks …). Values live in the item's custom fields under the field's
//! label; a template only says which labels to offer, how to edit and fill
//! them and which ones to conceal.

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::ItemKind;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/templates.rs"));
}

/// How a field is typed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldInput {
    /// One line of text.
    #[default]
    Text,
    /// Several lines (addresses, keys).
    Multiline,
    /// A calendar date.
    Date,
    /// Month and year (card expiry).
    MonthYear,
    /// An e-mail address.
    Email,
    /// A phone number.
    Phone,
    /// A web address.
    Url,
    /// A one-time password secret (shows the current code).
    Totp,
    /// "Sign in with": the id of the login used to sign in (Google, Apple …).
    SignInWith,
}

/// One field of a category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldSpec {
    /// Label, also the key the value is stored under. Never translated.
    pub label: String,
    /// How it is typed in.
    #[serde(default)]
    pub input: FieldInput,
    /// Hidden until revealed, like a password.
    #[serde(default)]
    pub concealed: bool,
    /// Section heading the field sits under (`""`: the item's first block).
    #[serde(default)]
    pub section: String,
    /// What forms call it: an HTML autocomplete name (`postal-code`) or a
    /// token from `templates/autofill.json` / the country's file (`cpf`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autofill: Option<String>,
    /// Translation key of the name shown (`street`), when it differs from the label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Input mask: `0` a digit, `A` a letter, `*` anything, other characters as is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    /// A named check of the value (`cpf`, `luhn` …), shown as a warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validate: Option<String>,
    /// Allowed values (a list to pick from).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

type Fields = HashMap<ItemKind, Vec<FieldSpec>>;

#[derive(Deserialize)]
struct RegionFile {
    #[serde(default)]
    categories: BTreeMap<String, Vec<FieldSpec>>,
}

struct Templates {
    base: Fields,
    regions: BTreeMap<String, Fields>,
}

fn by_kind(categories: BTreeMap<String, Vec<FieldSpec>>, file: &str) -> Fields {
    categories
        .into_iter()
        .filter(|(key, _)| !key.starts_with('$'))
        .map(|(key, fields)| {
            let kind = ItemKind::from_str(&key)
                .unwrap_or_else(|_| panic!("{file}: unknown category {key}"));
            (kind, fields)
        })
        .collect()
}

static TEMPLATES: LazyLock<Templates> = LazyLock::new(|| {
    let base: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(embedded::CATEGORIES).expect("templates/categories.json");
    let base = base
        .into_iter()
        .filter(|(key, _)| !key.starts_with('$'))
        .map(|(key, value)| {
            let fields = serde_json::from_value(value)
                .unwrap_or_else(|e| panic!("categories.json {key}: {e}"));
            (key, fields)
        })
        .collect();
    let regions = embedded::REGIONS
        .iter()
        .map(|(code, json)| {
            let file: RegionFile = serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("templates/regions/{code}.json: {e}"));
            ((*code).to_owned(), by_kind(file.categories, code))
        })
        .collect();
    Templates {
        base: by_kind(base, "categories.json"),
        regions,
    }
});

/// The extra fields of `kind` used everywhere. Logins, passwords and secure
/// notes use the built-in username / password / website / notes fields instead.
#[must_use]
pub fn template(kind: ItemKind) -> &'static [FieldSpec] {
    TEMPLATES.base.get(&kind).map_or(&[], Vec::as_slice)
}

/// The fields of `kind` in the country `region` (`"BR"`); the shared ones
/// when the country has none of its own (or `region` is empty / unknown).
#[must_use]
pub fn template_in(kind: ItemKind, region: &str) -> &'static [FieldSpec] {
    TEMPLATES
        .regions
        .get(region)
        .and_then(|r| r.get(&kind))
        .map_or_else(|| template(kind), Vec::as_slice)
}

/// Countries with their own fields, and for which categories.
#[must_use]
pub fn region_templates(kind: ItemKind) -> BTreeMap<&'static str, &'static [FieldSpec]> {
    TEMPLATES
        .regions
        .iter()
        .filter_map(|(code, fields)| fields.get(&kind).map(|f| (code.as_str(), f.as_slice())))
        .collect()
}

/// Every country that has a file.
#[must_use]
pub fn regions() -> Vec<&'static str> {
    TEMPLATES.regions.keys().map(String::as_str).collect()
}

/// The field called `label` (case-insensitive) of `kind` in `region`.
#[must_use]
pub fn spec_in(kind: ItemKind, region: &str, label: &str) -> Option<&'static FieldSpec> {
    template_in(kind, region)
        .iter()
        .find(|f| f.label.eq_ignore_ascii_case(label))
}

/// The shared template field called `label` (case-insensitive), if `kind` has one.
#[must_use]
pub fn spec(kind: ItemKind, label: &str) -> Option<&'static FieldSpec> {
    spec_in(kind, "", label)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn check(fields: &[FieldSpec], what: &str) {
        assert!(!fields.is_empty(), "{what}");
        let labels: HashSet<_> = fields.iter().map(|f| f.label.to_lowercase()).collect();
        assert_eq!(labels.len(), fields.len(), "{what}: labels must be unique");
        let mut seen = Vec::new();
        for f in fields {
            if seen.last() != Some(&f.section) {
                assert!(
                    !seen.contains(&f.section),
                    "{what}: section {} is split",
                    f.section
                );
                seen.push(f.section.clone());
            }
        }
    }

    #[test]
    fn every_template_is_well_formed() {
        // Documents are a file and notes: no fields of their own.
        for &kind in ItemKind::ALL[3..]
            .iter()
            .filter(|k| **k != ItemKind::Document)
        {
            check(template(kind), kind.as_str());
        }
        assert!(template(ItemKind::Login).is_empty());
        for code in regions() {
            assert!(
                code.len() == 2 && code.chars().all(|c| c.is_ascii_uppercase()),
                "{code}"
            );
            for &kind in ItemKind::ALL {
                if let Some(fields) = region_templates(kind).get(code) {
                    check(fields, &format!("{code} {kind}"));
                }
            }
        }
    }

    #[test]
    fn secrets_are_concealed() {
        let card = |l| spec(ItemKind::CreditCard, l).unwrap();
        assert!(card("Number").concealed && card("verification number").concealed);
        assert!(!card("cardholder name").concealed);
        assert_eq!(card("expiry date").input, FieldInput::MonthYear);
        assert!(spec(ItemKind::CreditCard, "nickname").is_none());
        assert!(spec(ItemKind::SshKey, "private key").unwrap().concealed);
    }

    #[test]
    fn identity_fields_are_grouped_like_1password() {
        let section = |l| spec(ItemKind::Identity, l).unwrap().section.as_str();
        assert_eq!(section("first name"), "Identification");
        assert_eq!(section("zip"), "Address");
        assert_eq!(section("website"), "Internet Details");
        assert_eq!(spec(ItemKind::CreditCard, "number").unwrap().section, "");
        assert_eq!(
            spec(ItemKind::Identity, "zip").unwrap().autofill.as_deref(),
            Some("postal-code")
        );
    }

    #[test]
    fn an_unknown_country_uses_the_shared_fields() {
        assert_eq!(
            template_in(ItemKind::Identity, "ZZ"),
            template(ItemKind::Identity)
        );
        assert_eq!(template_in(ItemKind::Login, ""), template(ItemKind::Login));
    }
}
