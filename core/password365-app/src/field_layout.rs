//! Sections and field types of an item's custom fields.
//!
//! The stored field is only a label, a value and a concealed flag. Which
//! section a field sits in and how it is typed in (1Password's "add another
//! field" menu) live in one internal custom field holding JSON, so they sync
//! with the fields themselves and older records need no new format. Only the
//! fields that differ from the category's template are listed there.

use std::collections::BTreeMap;

use password365_model::item_template::{FieldInput, spec_in};
use password365_model::{CustomField, ItemKind};
use serde::{Deserialize, Serialize};

/// Label of the internal field. Never shown, never accepted from a form.
pub const LAYOUT_LABEL: &str = "password365 field layout";

/// Where a field sits and how it is typed in.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldMeta {
    /// Section heading (`""`: the item's first block).
    pub section: String,
    /// Kind of input.
    pub input: FieldInput,
}

/// `true` for the internal layout field.
#[must_use]
pub fn is_internal(label: &str) -> bool {
    label.eq_ignore_ascii_case(LAYOUT_LABEL)
        || label.eq_ignore_ascii_case(crate::item_icon::ICON_LABEL)
        || label.eq_ignore_ascii_case(crate::passkeys::PASSKEY_LABEL)
}

/// What the template of `region` says about `label`, or a plain text field.
#[must_use]
pub fn default_meta(kind: ItemKind, region: &str, label: &str) -> FieldMeta {
    spec_in(kind, region, label).map_or_else(FieldMeta::default, |s| FieldMeta {
        section: s.section.clone(),
        input: s.input,
    })
}

/// What the internal field holds.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Layout {
    /// Country whose template the item follows (`""`: the shared one).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    region: String,
    /// Fields placed differently from that template.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    fields: BTreeMap<String, FieldMeta>,
}

fn stored(fields: &[CustomField]) -> Layout {
    fields
        .iter()
        .find(|f| is_internal(&f.label))
        .and_then(|f| serde_json::from_str(f.value.expose()).ok())
        .unwrap_or_default()
}

/// The country whose template the item follows (`""`: the shared one).
#[must_use]
pub fn region_of(fields: &[CustomField]) -> String {
    stored(fields).region
}

/// The item's visible fields, each with its section and input.
#[must_use]
pub fn describe(kind: ItemKind, fields: &[CustomField]) -> Vec<(&CustomField, FieldMeta)> {
    let layout = stored(fields);
    fields
        .iter()
        .filter(|f| !is_internal(&f.label))
        .map(|f| {
            let meta = layout
                .fields
                .get(&f.label)
                .cloned()
                .unwrap_or_else(|| default_meta(kind, &layout.region, &f.label));
            (f, meta)
        })
        .collect()
}

/// The internal field for `fields` of an item following `region`'s template
/// (`None` when it follows the shared template exactly).
#[must_use]
pub fn layout_field(
    kind: ItemKind,
    region: &str,
    fields: &[(&str, FieldMeta)],
) -> Option<CustomField> {
    let layout = Layout {
        region: region.to_owned(),
        fields: fields
            .iter()
            .filter(|(label, meta)| *meta != default_meta(kind, region, label))
            .map(|(label, meta)| ((*label).to_owned(), meta.clone()))
            .collect(),
    };
    (!layout.region.is_empty() || !layout.fields.is_empty()).then(|| CustomField {
        label: LAYOUT_LABEL.to_owned(),
        value: serde_json::to_string(&layout).unwrap_or_default().into(),
        concealed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(label: &str, value: &str) -> CustomField {
        CustomField {
            label: label.into(),
            value: value.into(),
            concealed: false,
        }
    }

    fn meta(section: &str, input: FieldInput) -> FieldMeta {
        FieldMeta {
            section: section.into(),
            input,
        }
    }

    #[test]
    fn template_fields_need_no_layout() {
        let m = default_meta(ItemKind::Identity, "", "zip");
        assert_eq!(m, meta("Address", FieldInput::Text));
        assert!(layout_field(ItemKind::Identity, "", &[("zip", m)]).is_none());
        assert_eq!(
            default_meta(ItemKind::Login, "", "pet"),
            FieldMeta::default()
        );
    }

    #[test]
    fn the_country_is_kept_and_its_template_used() {
        let kind = ItemKind::Identity;
        let cpf = default_meta(kind, "BR", "CPF");
        assert_eq!(cpf.section, "Identification");
        let layout = layout_field(kind, "BR", &[("CPF", cpf)]).unwrap();
        assert!(!layout.value.expose().contains("CPF"));
        let fields = [field("CPF", "1"), layout];
        assert_eq!(region_of(&fields), "BR");
        assert_eq!(describe(kind, &fields)[0].1.section, "Identification");
        assert_eq!(region_of(&[field("x", "y")]), "");
    }

    #[test]
    fn user_fields_round_trip_through_the_layout() {
        let kind = ItemKind::Identity;
        let wife = meta("Family", FieldInput::Phone);
        let layout = layout_field(
            kind,
            "",
            &[
                ("zip", default_meta(kind, "", "zip")),
                ("wife", wife.clone()),
            ],
        )
        .unwrap();
        assert!(is_internal(&layout.label));
        assert!(!layout.value.expose().contains("zip"));
        let fields = [field("zip", "1"), field("wife", "555"), layout];
        let described = describe(kind, &fields);
        assert_eq!(described.len(), 2);
        assert_eq!(described[0].1.section, "Address");
        assert_eq!(described[1].1, wife);
    }

    #[test]
    fn a_broken_layout_falls_back_to_the_template() {
        let fields = [field("zip", "1"), field(LAYOUT_LABEL, "{not json")];
        let described = describe(ItemKind::Identity, &fields);
        assert_eq!(described.len(), 1);
        assert_eq!(described[0].1.section, "Address");
    }
}
