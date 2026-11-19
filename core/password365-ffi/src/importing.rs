//! Import from another manager: the phone is handed the file the source app
//! made (a browser or Apple Passwords CSV, 1Password `.1pux`, Credential
//! Exchange `.cxf`/`.json`, Bitwarden `.json`, `KeePass` `.kdbx`) and reads it
//! with the same importers as the desktop, into the default vault.

use std::io::Read;
use std::path::Path;

use password365_app::AppError;
use password365_app::dto::ImportSummary;
use password365_crypto::secret::SecretString;

use crate::{Failure, Password365, Result};

/// What importing a file did.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ImportResult {
    /// New items created.
    pub created: u64,
    /// Entries skipped because the vault (or the file itself) already had them.
    pub duplicates: u64,
    /// Entries that could not be read.
    pub failed: u64,
    /// The first reasons entries failed, as `line N: reason` (English, for support).
    pub failures: Vec<String>,
}

/// How many failure reasons cross to the screen.
const MAX_FAILURES: usize = 5;

impl From<ImportSummary> for ImportResult {
    fn from(s: ImportSummary) -> Self {
        Self {
            created: s.imported as u64,
            duplicates: (s.identical + s.changed + s.duplicates_in_file) as u64,
            failed: s.skipped.len() as u64,
            failures: s
                .skipped
                .iter()
                .take(MAX_FAILURES)
                .map(|(line, reason)| format!("line {line}: {reason}"))
                .collect(),
        }
    }
}

/// The kinds of file the importers read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Csv,
    OnePux,
    Cxf,
    Bitwarden,
    Kdbx,
}

impl Format {
    /// By extension; a `.json` is a Credential Exchange file when it has
    /// the `accounts` list those carry, otherwise Bitwarden's export.
    fn detect(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        Ok(match ext.as_str() {
            "1pux" => Self::OnePux,
            "kdbx" => Self::Kdbx,
            "cxf" => Self::Cxf,
            "json" => {
                if Self::json_has_accounts(path)? {
                    Self::Cxf
                } else {
                    Self::Bitwarden
                }
            }
            _ => Self::Csv,
        })
    }

    fn json_has_accounts(path: &Path) -> Result<bool> {
        let mut text = String::new();
        std::fs::File::open(path)
            .and_then(|f| f.take(64 * 1024 * 1024).read_to_string(&mut text))
            .map_err(|e| AppError::Io(format!("cannot read {}: {e}", path.display())))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|_| AppError::Validation("the file is not valid JSON".into()))?;
        Ok(value.get("accounts").is_some())
    }
}

#[uniffi::export]
impl Password365 {
    /// Imports the file at `path` into the default vault, choosing the
    /// importer by its extension and content. A `.kdbx` needs its
    /// `password`. Items already in the vault are not duplicated.
    pub fn import_file(&self, path: String, password: Option<String>) -> Result<ImportResult> {
        let path = Path::new(&path);
        let format = Format::detect(path)?;
        let mut app = self.app();
        let summary = match format {
            Format::Csv => app.import_csv(path, None, false),
            Format::OnePux => app.import_1pux(path, None),
            Format::Cxf => app.import_cxf(path, None),
            Format::Bitwarden => app.import_bitwarden(path, None),
            Format::Kdbx => {
                let Some(password) = password.filter(|p| !p.is_empty()) else {
                    return Err(Failure::Failed {
                        code: "password_required".into(),
                        reason: "this database needs its password".into(),
                    });
                };
                app.import_kdbx(path, &SecretString::from(password), None, None)
            }
        }?;
        Ok(summary.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(name: &str, content: &str) -> Format {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        Format::detect(&path).unwrap()
    }

    #[test]
    fn detects_by_extension_and_content() {
        assert_eq!(detect("Passwords.csv", ""), Format::Csv);
        assert_eq!(detect("noextension", ""), Format::Csv);
        assert_eq!(detect("export.1PUX", ""), Format::OnePux);
        assert_eq!(detect("db.kdbx", ""), Format::Kdbx);
        assert_eq!(detect("x.cxf", ""), Format::Cxf);
        assert_eq!(
            detect("x.json", r#"{"version":{},"accounts":[]}"#),
            Format::Cxf
        );
        assert_eq!(detect("x.json", r#"{"items":[]}"#), Format::Bitwarden);
    }

    #[test]
    fn a_json_that_is_not_json_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.json");
        std::fs::write(&path, "nope").unwrap();
        assert!(Format::detect(&path).is_err());
    }

    #[test]
    fn summary_counts_everything_skipped_as_duplicates() {
        let result = ImportResult::from(ImportSummary {
            imported: 2,
            identical: 1,
            changed: 1,
            updated: 0,
            duplicates_in_file: 1,
            skipped: (1..=7).map(|n| (n, "bad".to_owned())).collect(),
        });
        assert_eq!(
            (result.created, result.duplicates, result.failed),
            (2, 3, 7)
        );
        assert_eq!(result.failures.len(), MAX_FAILURES);
        assert_eq!(result.failures[0], "line 1: bad");
    }
}

#[cfg(test)]
mod import_tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::tests::{MemoryKeeper, PASSWORD, vault};

    use super::*;

    const CHROME_CSV: &str = "name,url,username,password,note\n\
        GitHub,https://github.com/login,octo,s3cret,\n\
        Mail,https://mail.example.com,ana,pw2,hi\n\
        GitHub again,https://github.com/login,octo,s3cret,\n\
        ,,,,\n";
    const BITWARDEN: &str = r#"{"encrypted":false,"items":[
      {"type":1,"name":"Bank","login":{"username":"ana","password":"pw","uris":[{"uri":"https://bank.example"}]}}]}"#;
    const CXF: &str = r#"{"version":{"major":1,"minor":0},"exporterRpId":"x","exporterDisplayName":"X",
      "timestamp":1755129600,"accounts":[{"id":"QQ","username":"","email":"","collections":[],"items":[
        {"id":"AQ","title":"Forum","scope":{"urls":["https://forum.example"],"androidApps":[]},
         "credentials":[{"type":"basic-auth","username":{"fieldType":"string","value":"u"},
                         "password":{"fieldType":"concealed-string","value":"p"}}]}]}]}"#;

    fn unlocked() -> (tempfile::TempDir, Arc<Password365>) {
        let dir = tempfile::tempdir().unwrap();
        let p = vault(&dir, Arc::new(MemoryKeeper::default()));
        p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
        (dir, p)
    }

    fn file(dir: &tempfile::TempDir, name: &str, content: &str) -> String {
        let path: PathBuf = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path.to_str().unwrap().to_owned()
    }

    #[test]
    fn imports_a_browser_csv_and_skips_what_is_already_there() {
        let (dir, p) = unlocked();
        let path = file(&dir, "Passwords.csv", CHROME_CSV);
        let first = p.import_file(path.clone(), None).unwrap();
        assert_eq!((first.created, first.duplicates), (2, 1));
        assert_eq!(first.failed, 1);
        assert_eq!(first.failures.len(), 1);
        assert_eq!(p.items(String::new()).unwrap().len(), 2);

        let again = p.import_file(path, None).unwrap();
        assert_eq!((again.created, again.duplicates), (0, 3));
        assert_eq!(p.items(String::new()).unwrap().len(), 2);
    }

    #[test]
    fn imports_bitwarden_and_credential_exchange_json() {
        let (dir, p) = unlocked();
        let bw = p
            .import_file(file(&dir, "bitwarden.json", BITWARDEN), None)
            .unwrap();
        assert_eq!(bw.created, 1);
        let cxf = p.import_file(file(&dir, "cxf.json", CXF), None).unwrap();
        assert_eq!(cxf.created, 1);
        let cxf2 = p
            .import_file(file(&dir, "Passwords.cxf", CXF), None)
            .unwrap();
        assert_eq!((cxf2.created, cxf2.duplicates), (0, 1));
    }

    #[test]
    fn a_kdbx_without_its_password_asks_for_it() {
        let (dir, p) = unlocked();
        let path = file(&dir, "db.kdbx", "not a database");
        for password in [None, Some(String::new())] {
            let Err(Failure::Failed { code, .. }) = p.import_file(path.clone(), password) else {
                panic!("should fail")
            };
            assert_eq!(code, "password_required");
        }
        assert!(
            p.import_file(path, Some("pw".into())).is_err(),
            "not a real database"
        );
    }

    #[test]
    fn refuses_a_garbage_file_and_a_missing_one() {
        let (dir, p) = unlocked();
        assert!(p.import_file(file(&dir, "x.1pux", "zip?"), None).is_err());
        assert!(p.import_file(file(&dir, "x.json", "nope"), None).is_err());
        assert!(
            p.import_file(dir.path().join("none.csv").to_str().unwrap().into(), None)
                .is_err()
        );
    }

    #[test]
    fn needs_an_unlocked_vault() {
        let (dir, p) = unlocked();
        p.lock();
        let path = file(&dir, "a.csv", CHROME_CSV);
        assert!(p.import_file(path, None).is_err());
    }
}
