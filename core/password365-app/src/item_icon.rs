//! An item's own icon ("Choose new icon…"): a small image the user picked,
//! kept as a data URL in an internal custom field so it syncs with the item.

use std::collections::BTreeMap;

use password365_model::CustomField;

use crate::{AppError, AppService, Result};

/// Label of the internal field holding the icon. Never shown, never accepted from a form.
pub const ICON_LABEL: &str = "password365 icon";
/// Largest accepted icon (the app scales pictures down to 64×64 before saving).
pub const MAX_ICON_BYTES: usize = 96 * 1024;
const PREFIXES: [&str; 3] = [
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/webp;base64,",
];

/// The item's icon (a data URL), if it has one.
#[must_use]
pub fn icon_of(fields: &[CustomField]) -> Option<&str> {
    fields
        .iter()
        .find(|f| f.label.eq_ignore_ascii_case(ICON_LABEL))
        .map(|f| f.value.expose())
}

/// A gallery icon and its colour: `builtin:home:#16a34a` (names from `icons/icons.json`).
fn is_builtin(icon: &str) -> bool {
    let Some(rest) = icon.strip_prefix("builtin:") else {
        return false;
    };
    let (name, color) = rest.split_once(':').unwrap_or((rest, ""));
    let name_ok = !name.is_empty()
        && name.len() <= 40
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    let color_ok = color.is_empty()
        || (color.len() == 7
            && color.starts_with('#')
            && color[1..].bytes().all(|b| b.is_ascii_hexdigit()));
    name_ok && color_ok
}

/// Checks that `icon` is a gallery icon or a small PNG, JPEG or WebP data URL.
pub fn validate(icon: &str) -> Result<()> {
    if is_builtin(icon) {
        return Ok(());
    }
    let data = PREFIXES
        .iter()
        .find_map(|p| icon.strip_prefix(p))
        .ok_or_else(|| AppError::Validation("the icon must be a PNG, JPEG or WebP image".into()))?;
    let base64 = data
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='));
    if !base64 || icon.len() > MAX_ICON_BYTES {
        return Err(AppError::Validation(
            "the icon is too large or damaged".into(),
        ));
    }
    Ok(())
}

/// Replaces (`Some`) or removes (`None`) the icon in `fields`.
pub fn set_icon(fields: &mut Vec<CustomField>, icon: Option<&str>) -> Result<()> {
    if let Some(icon) = icon {
        validate(icon)?;
    }
    fields.retain(|f| !f.label.eq_ignore_ascii_case(ICON_LABEL));
    if let Some(icon) = icon {
        fields.push(CustomField {
            label: ICON_LABEL.to_owned(),
            value: icon.into(),
            concealed: false,
        });
    }
    Ok(())
}

impl AppService {
    /// Every item's own icon, by item id (the list shows them).
    pub fn item_icons(&mut self) -> Result<BTreeMap<String, String>> {
        let session = self.session()?;
        let mut icons = BTreeMap::new();
        for summary in session.items() {
            let item = session.item(summary.id)?;
            if let Some(icon) = icon_of(&item.details.custom_fields) {
                icons.insert(summary.id.to_string(), icon.to_owned());
            }
        }
        Ok(icons)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "data:image/png;base64,iVBORw0KGgo=";

    #[test]
    fn icons_are_validated_set_and_removed() {
        let mut fields = vec![CustomField {
            label: "pet".into(),
            value: "Rex".into(),
            concealed: false,
        }];
        set_icon(&mut fields, Some(PNG)).unwrap();
        set_icon(&mut fields, Some(PNG)).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(icon_of(&fields), Some(PNG));
        set_icon(&mut fields, None).unwrap();
        assert_eq!(icon_of(&fields), None);
        assert_eq!(fields.len(), 1);
        for bad in [
            "data:image/svg+xml;base64,PHN2Zz4=",
            "data:image/png;base64,<script>",
            "javascript:alert(1)",
        ] {
            assert!(set_icon(&mut fields, Some(bad)).is_err(), "{bad}");
        }
        let huge = format!("data:image/png;base64,{}", "A".repeat(MAX_ICON_BYTES));
        assert!(validate(&huge).is_err());
        for good in ["builtin:home", "builtin:bank_account:#16a34a"] {
            assert!(validate(good).is_ok(), "{good}");
        }
        for bad in [
            "builtin:",
            "builtin:Home",
            "builtin:home:red",
            "builtin:../x",
            "builtin:home:#12345g",
        ] {
            assert!(validate(bad).is_err(), "{bad}");
        }
    }
}
