//! Parsing browser password CSV exports.

use std::io::Read;

use csv::{ReaderBuilder, StringRecord};

use crate::columns::{Columns, Field};
use crate::{ImportError, ImportedLogin, Result};

/// A row that was not imported, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedRow {
    /// 1-based line number in the file (header = line 1).
    pub line: u64,
    /// Why it was skipped.
    pub reason: &'static str,
}

/// Result of parsing a CSV file.
#[derive(Debug, Default)]
pub struct ParsedCsv {
    /// Logins that can be imported.
    pub logins: Vec<ImportedLogin>,
    /// Rows that were skipped.
    pub skipped: Vec<SkippedRow>,
}

/// Parses a Chrome-compatible CSV (`name,url,username,password,note`) or a
/// similar export from Firefox, Safari, Edge, Brave or Bitwarden.
pub fn parse<R: Read>(reader: R) -> Result<ParsedCsv> {
    let mut csv = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::None)
        .from_reader(reader);
    let columns = Columns::from_headers(csv.headers()?);
    if !columns.has_password() {
        return Err(ImportError::MissingColumn("password"));
    }

    let mut parsed = ParsedCsv::default();
    for (index, row) in csv.records().enumerate() {
        let line = u64::try_from(index).unwrap_or(u64::MAX).saturating_add(2);
        match row {
            Ok(row) => match login_from_row(&columns, &row) {
                Some(login) => parsed.logins.push(login),
                None => parsed.skipped.push(SkippedRow {
                    line,
                    reason: "no username, password or URL",
                }),
            },
            Err(_) => parsed.skipped.push(SkippedRow {
                line,
                reason: "malformed CSV row",
            }),
        }
    }
    Ok(parsed)
}

fn login_from_row(columns: &Columns, row: &StringRecord) -> Option<ImportedLogin> {
    let text = |f| columns.get(row, f).map(str::to_owned);
    let secret = |f| columns.get(row, f).map(Into::into);
    // LastPass marks secure notes with this pseudo-URL.
    let url = text(Field::Url).filter(|u| u != "http://sn");
    let mut login = ImportedLogin {
        title: text(Field::Title).unwrap_or_default(),
        url,
        username: text(Field::Username),
        password: secret(Field::Password),
        notes: secret(Field::Notes),
        totp: secret(Field::Totp),
        tags: columns.get(row, Field::Tags).map(tags).unwrap_or_default(),
        favorite: columns
            .get(row, Field::Favorite)
            .is_some_and(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "x" | "y")),
        note_only: false,
    };
    let useful = login.password.is_some() || login.username.is_some() || login.url.is_some();
    login.note_only = !useful && login.notes.is_some() && !login.title.trim().is_empty();
    (useful || login.note_only).then_some(login)
}

/// Tags from a folder path or list: `Root/Internet` → `Internet`; `a, b` → `a`, `b`.
fn tags(value: &str) -> Vec<String> {
    value
        .split([',', ';'])
        .map(|t| {
            t.trim()
                .trim_start_matches("Root/")
                .trim_matches(['/', '\\'])
                .trim()
        })
        .filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("root"))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chrome_export() {
        let csv = "name,url,username,password,note\n\
                   github.com,https://github.com/login,octo,s3cret,my note\n\
                   ,https://www.example.com/,bob,pw2,\n";
        let parsed = parse(csv.as_bytes()).unwrap();
        assert!(parsed.skipped.is_empty());
        assert_eq!(parsed.logins.len(), 2);
        let first = &parsed.logins[0];
        assert_eq!(first.title, "github.com");
        assert_eq!(first.username.as_deref(), Some("octo"));
        assert_eq!(first.password.as_ref().unwrap().expose(), "s3cret");
        assert_eq!(first.notes.as_ref().unwrap().expose(), "my note");
        assert_eq!(parsed.logins[1].effective_title(), "example.com");
    }

    #[test]
    fn handles_quotes_commas_and_newlines() {
        let csv = "name,url,username,password,note\n\"A, B\",https://a.b,\"u\",\"p,\"\"q\",\"line1\nline2\"\n";
        let login = &parse(csv.as_bytes()).unwrap().logins[0];
        assert_eq!(login.title, "A, B");
        assert_eq!(login.password.as_ref().unwrap().expose(), "p,\"q");
        assert_eq!(login.notes.as_ref().unwrap().expose(), "line1\nline2");
    }

    #[test]
    fn preserves_password_whitespace_inside() {
        let csv = "name,url,username,password\nx,https://x.y,u,\"a b\"\n";
        assert_eq!(
            parse(csv.as_bytes()).unwrap().logins[0]
                .password
                .as_ref()
                .unwrap()
                .expose(),
            "a b"
        );
    }

    #[test]
    fn skips_empty_rows_with_line_numbers() {
        let csv = "name,url,username,password\nonly title,,,\nok,https://ok.com,u,p\n";
        let parsed = parse(csv.as_bytes()).unwrap();
        assert_eq!(parsed.logins.len(), 1);
        assert_eq!(
            parsed.skipped,
            vec![SkippedRow {
                line: 2,
                reason: "no username, password or URL"
            }]
        );
    }

    #[test]
    fn rejects_csv_without_password_column() {
        let err = parse("a,b\n1,2\n".as_bytes()).unwrap_err();
        assert!(matches!(err, ImportError::MissingColumn("password")));
    }

    #[test]
    fn invalid_utf8_row_is_skipped() {
        let mut bytes = b"name,url,username,password\n".to_vec();
        bytes.extend_from_slice(b"\xff\xfe,https://a.com,u,p\nok,https://b.com,u,p\n");
        let parsed = parse(bytes.as_slice()).unwrap();
        assert_eq!(parsed.logins.len(), 1);
        assert_eq!(parsed.skipped[0].reason, "malformed CSV row");
    }

    #[test]
    fn keeps_folders_favorites_and_notes() {
        let csv = "url,username,password,totp,extra,name,grouping,fav\n\
                   https://a.com,u,p,,,A,Bank,1\n\
                   http://sn,,,,Wi-Fi key 123,Wi-Fi,Root/Home,0\n\
                   ,,,,,empty,,\n";
        let parsed = parse(csv.as_bytes()).unwrap();
        assert_eq!(parsed.logins.len(), 2);
        assert_eq!(parsed.logins[0].tags, vec!["Bank"]);
        assert!(parsed.logins[0].favorite);
        let note = &parsed.logins[1];
        assert!(note.note_only && note.url.is_none());
        assert_eq!(note.tags, vec!["Home"]);
        assert_eq!(parsed.skipped.len(), 1);
        assert_eq!(tags("a, b;Root"), vec!["a", "b"]);
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(parse("".as_bytes()).is_err());
    }
}
