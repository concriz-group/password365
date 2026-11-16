//! Writing items as a Chrome-compatible password CSV.
//!
//! Format: `name,url,username,password,note` — importable at
//! `chrome://password-manager/settings`. The output is **plaintext**. Cells
//! that a spreadsheet would run as formulas are made inert (except passwords).

use std::io::Write;

use password365_crypto::secret::SecretString;
use password365_model::{Item, ItemKind};

use crate::Result;

/// Chrome's CSV header.
pub const HEADER: [&str; 5] = ["name", "url", "username", "password", "note"];

/// Writes all Login/Password items that have a password. Returns the number of rows written.
pub fn write<W: Write>(writer: W, items: &[Item]) -> Result<usize> {
    let mut csv = csv::Writer::from_writer(writer);
    csv.write_record(HEADER)?;
    let mut written = 0;
    for item in items.iter().filter(|i| is_exportable(i)) {
        csv.write_record([
            inert(&item.overview.title).as_ref(),
            inert(item.overview.urls.first().map_or("", String::as_str)).as_ref(),
            inert(item.overview.username.as_deref().unwrap_or_default()).as_ref(),
            // Never changed: the file must import back with the same password.
            expose(item.details.password.as_ref()),
            inert(expose(item.details.notes.as_ref())).as_ref(),
        ])?;
        written += 1;
    }
    csv.flush().map_err(csv::Error::from)?;
    Ok(written)
}

/// `text` made inert for spreadsheets: a cell starting with `=`, `+`, `-`,
/// `@`, a tab or a carriage return would run as a formula when the file is
/// opened in Excel or Sheets, so it gets a leading `'` (passwords excepted).
fn inert(text: &str) -> std::borrow::Cow<'_, str> {
    if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}").into()
    } else {
        text.into()
    }
}

fn is_exportable(item: &Item) -> bool {
    matches!(item.kind, ItemKind::Login | ItemKind::Password) && item.details.password.is_some()
}

fn expose(secret: Option<&SecretString>) -> &str {
    secret.map_or("", SecretString::expose)
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemOverview, Timestamp, VaultId};

    use super::*;
    use crate::csv_import;

    fn item(kind: ItemKind, title: &str, pw: Option<&str>) -> Item {
        let details = pw.map_or_else(ItemDetails::default, |p| {
            ItemDetails::with_password(p.into())
        });
        Item::new(
            VaultId::new(),
            kind,
            ItemOverview::new(title)
                .unwrap()
                .with_url("https://site.com")
                .with_username("me"),
            details.notes("a, \"note\"".into()),
            Timestamp::now(),
        )
    }

    #[test]
    fn formulas_are_made_inert_but_passwords_are_kept() {
        let mut evil = item(ItemKind::Login, "=HYPERLINK(\"http://x\")", Some("=pass"));
        evil.overview.username = Some("@SUM(1)".into());
        let mut out = Vec::new();
        write(&mut out, &[evil]).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"'=HYPERLINK(\"\"http://x\"\")\""), "{text}");
        assert!(text.contains(",'@SUM(1),=pass,"), "{text}");
        assert_eq!(inert("plain"), "plain");
        assert_eq!(inert("-1"), "'-1");
    }

    #[test]
    fn writes_header_and_exportable_rows() {
        let items = vec![
            item(ItemKind::Login, "Site", Some("pw")),
            item(ItemKind::SecureNote, "Note", None),
            item(ItemKind::Login, "No password", None),
            item(ItemKind::Password, "Wifi", Some("w")),
        ];
        let mut out = Vec::new();
        assert_eq!(write(&mut out, &items).unwrap(), 2);
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("name,url,username,password,note\n"));
        assert!(text.contains("Site,https://site.com,me,pw,"));
        assert!(!text.contains("Note,"));
    }

    #[test]
    fn export_then_import_round_trips() {
        let items = vec![item(ItemKind::Login, "Site, Inc.", Some(" p\"w, "))];
        let mut out = Vec::new();
        write(&mut out, &items).unwrap();
        let parsed = csv_import::parse(out.as_slice()).unwrap();
        let login = &parsed.logins[0];
        assert_eq!(login.title, "Site, Inc.");
        assert_eq!(login.password.as_ref().unwrap().expose(), " p\"w, ");
        assert_eq!(login.notes.as_ref().unwrap().expose(), "a, \"note\"");
    }

    #[test]
    fn empty_list_writes_only_header() {
        let mut out = Vec::new();
        assert_eq!(write(&mut out, &[]).unwrap(), 0);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "name,url,username,password,note\n"
        );
    }
}
