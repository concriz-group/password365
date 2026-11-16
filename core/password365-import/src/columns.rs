//! Locating fields in CSV headers exported by different browsers / managers.
//! The header names live in the repository's `templates/imports.json`.

use std::collections::HashMap;
use std::sync::LazyLock;

use csv::StringRecord;
use serde::Deserialize;

/// Logical fields of an imported row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Field {
    Title,
    Url,
    Username,
    Password,
    Notes,
    Totp,
    Tags,
    Favorite,
}

#[derive(Deserialize)]
struct ImportsFile {
    columns: HashMap<Field, Vec<String>>,
}

/// Header names of each field, most specific first.
static ALIASES: LazyLock<HashMap<Field, Vec<String>>> = LazyLock::new(|| {
    let file: ImportsFile = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../templates/imports.json"
    )))
    .expect("templates/imports.json");
    file.columns
});

/// Column positions resolved from a header row.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Columns {
    positions: HashMap<Field, usize>,
}

impl Columns {
    /// Resolves columns from the header row: for each field, the first of its
    /// names (in preference order) that the file has.
    pub(crate) fn from_headers(headers: &StringRecord) -> Self {
        let position = |name: &str| {
            headers
                .iter()
                .position(|h| h.trim().eq_ignore_ascii_case(name))
        };
        let positions = ALIASES
            .iter()
            .filter_map(|(field, names)| {
                names.iter().find_map(|n| position(n)).map(|p| (*field, p))
            })
            .collect();
        Self { positions }
    }

    /// `true` if a password column exists.
    pub(crate) fn has_password(&self) -> bool {
        self.positions.contains_key(&Field::Password)
    }

    /// Non-empty value of `field` in `row`. Text fields are trimmed; secrets
    /// (password, TOTP) are returned byte-exact because spaces may be significant.
    pub(crate) fn get<'r>(&self, row: &'r StringRecord, field: Field) -> Option<&'r str> {
        let value = row.get(*self.positions.get(&field)?)?;
        let value = if matches!(field, Field::Password | Field::Totp) {
            value
        } else {
            value.trim()
        };
        (!value.is_empty()).then_some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(fields: &[&str]) -> StringRecord {
        StringRecord::from(fields.to_vec())
    }

    #[test]
    fn resolves_chrome_headers() {
        let cols = Columns::from_headers(&record(&["name", "url", "username", "password", "note"]));
        let row = record(&["GitHub", "https://github.com", "octo", "pw", ""]);
        assert!(cols.has_password());
        assert_eq!(cols.get(&row, Field::Title), Some("GitHub"));
        assert_eq!(cols.get(&row, Field::Url), Some("https://github.com"));
        assert_eq!(cols.get(&row, Field::Username), Some("octo"));
        assert_eq!(cols.get(&row, Field::Password), Some("pw"));
        assert_eq!(cols.get(&row, Field::Notes), None, "empty → None");
        assert_eq!(cols.get(&row, Field::Totp), None, "missing column → None");
    }

    #[test]
    fn resolves_safari_and_bitwarden_style_headers() {
        let safari = Columns::from_headers(&record(&[
            "Title", "URL", "Username", "Password", "Notes", "OTPAuth",
        ]));
        let row = record(&["t", "u", "n", "p", "x", "otpauth://totp/a"]);
        assert_eq!(safari.get(&row, Field::Totp), Some("otpauth://totp/a"));
        assert_eq!(safari.get(&row, Field::Notes), Some("x"));

        let bw = Columns::from_headers(&record(&[
            "name",
            "login_uri",
            "login_username",
            "login_password",
        ]));
        assert_eq!(
            bw.get(&record(&["a", "b", "c", "d"]), Field::Password),
            Some("d")
        );
    }

    #[test]
    fn secrets_are_not_trimmed_but_text_is() {
        let cols = Columns::from_headers(&record(&["name", "password"]));
        let row = record(&["  Title  ", "  pw  "]);
        assert_eq!(cols.get(&row, Field::Title), Some("Title"));
        assert_eq!(cols.get(&row, Field::Password), Some("  pw  "));
    }

    #[test]
    fn prefers_the_more_specific_name() {
        // Proton Pass: email before username; the username wins.
        let cols = Columns::from_headers(&record(&[
            "name", "url", "email", "username", "password", "vault",
        ]));
        let row = record(&["a", "b", "me@x.com", "me", "p", "Work"]);
        assert_eq!(cols.get(&row, Field::Username), Some("me"));
        assert_eq!(cols.get(&row, Field::Tags), Some("Work"));
        // RoboForm.
        let robo =
            Columns::from_headers(&record(&["Name", "Url", "Login", "Pwd", "Note", "Folder"]));
        assert!(robo.has_password());
        assert_eq!(
            robo.get(
                &record(&["a", "b", "c", "d", "e", "/Bank"]),
                Field::Username
            ),
            Some("c")
        );
    }

    #[test]
    fn missing_password_column_detected() {
        assert!(!Columns::from_headers(&record(&["foo", "bar"])).has_password());
    }

    #[test]
    fn short_rows_are_tolerated() {
        let cols = Columns::from_headers(&record(&["name", "url", "username", "password"]));
        assert_eq!(cols.get(&record(&["only-name"]), Field::Password), None);
    }
}
