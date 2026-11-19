//! Importing other managers' full exports into a vault, every category with
//! sections, tags, favorites and the archive: 1Password (`.1pux`) and
//! Bitwarden (unencrypted `.json`).

use std::fs::File;
use std::path::Path;

use password365_crypto::secret::SecretString;
use password365_import::item::ImportedItem;
use password365_import::{bitwarden, cxf, kdbx, onepux};
use password365_model::ItemKind;
use password365_model::item_template::FieldInput;

use crate::dto::ImportSummary;
use crate::item_input::{FieldValue, ItemInput};
use crate::{AppError, AppService, Result};

/// Title for an item the export left untitled.
const UNTITLED: &str = "Imported item";

fn input_kind(name: &str) -> FieldInput {
    match name {
        "date" => FieldInput::Date,
        "month_year" => FieldInput::MonthYear,
        "email" => FieldInput::Email,
        "phone" => FieldInput::Phone,
        "url" => FieldInput::Url,
        "multiline" => FieldInput::Multiline,
        "totp" => FieldInput::Totp,
        _ => FieldInput::Text,
    }
}

fn valid_totp(secret: &str) -> bool {
    password365_crypto::totp::Totp::parse(secret).is_ok()
}

/// The form the item would have been typed into.
fn to_input(item: &ImportedItem, vault: &str) -> ItemInput {
    let mut fields: Vec<FieldValue> = item
        .fields
        .iter()
        .map(|f| {
            // A one-time password the app cannot read stays as text, not lost.
            let input = match input_kind(f.input) {
                FieldInput::Totp if !valid_totp(&f.value) => FieldInput::Text,
                other => other,
            };
            FieldValue {
                label: f.label.clone(),
                value: SecretString::from(f.value.as_str()),
                section: Some(f.section.clone()),
                input: Some(input),
                concealed: Some(f.concealed),
            }
        })
        .collect();
    let totp = item.totp.as_deref().filter(|t| {
        let ok = valid_totp(t);
        if !ok {
            fields.push(FieldValue {
                label: "one-time password".into(),
                value: (*t).into(),
                concealed: Some(true),
                ..FieldValue::default()
            });
        }
        ok
    });
    ItemInput {
        vault_id: Some(vault.to_owned()),
        kind: Some(item.kind.as_str().to_owned()),
        title: if item.title.trim().is_empty() {
            UNTITLED.into()
        } else {
            item.title.clone()
        },
        username: item.username.clone(),
        urls: item.urls.clone(),
        tags: item.tags.clone(),
        favorite: item.favorite,
        password: item.password.as_deref().map(SecretString::from),
        notes: item.notes.as_deref().map(SecretString::from),
        totp: totp.map(SecretString::from),
        fields: (!fields.is_empty()).then_some(fields),
        ..ItemInput::default()
    }
}

/// What identifies a login: each of its sites (origin) with the lowercase username.
pub(crate) fn login_keys(urls: &[String], username: Option<&str>) -> Vec<(String, String)> {
    let user = username.unwrap_or_default().trim().to_lowercase();
    urls.iter()
        .map(|u| password365_import::url_key::origin_key(u))
        .filter(|o| !o.is_empty())
        .map(|o| (o, user.clone()))
        .collect()
}

impl AppService {
    /// Imports a `.1pux` file into `vault_id` (the default vault when `None`).
    /// Items already there (same category, title and username) are skipped.
    pub fn import_1pux(&mut self, path: &Path, vault_id: Option<&str>) -> Result<ImportSummary> {
        let file = File::open(path)
            .map_err(|e| AppError::Io(format!("cannot open {}: {e}", path.display())))?;
        let export = onepux::read(file)?;
        let (mut summary, created) = self.import_items(&export.items, export.skipped, vault_id)?;
        for file in &export.files {
            // Items that were already here keep the files they have.
            let Some(Some(id)) = created.get(file.item) else {
                continue;
            };
            let item = crate::service::parse_id::<password365_model::ItemId>(id)?;
            let content_type = crate::attachments::content_type_of(&file.name);
            if let Err(e) = self
                .session()?
                .attach(item, &file.name, content_type, &file.bytes)
            {
                let line = u64::try_from(file.item + 1).unwrap_or(u64::MAX);
                summary.skipped.push((line, format!("{}: {e}", file.name)));
            }
        }
        Ok(summary)
    }

    /// Imports a CXF file (the standard credential exchange format) into
    /// `vault_id`, passkeys included.
    pub fn import_cxf(&mut self, path: &Path, vault_id: Option<&str>) -> Result<ImportSummary> {
        let json = zeroize::Zeroizing::new(
            std::fs::read_to_string(path)
                .map_err(|e| AppError::Io(format!("cannot open {}: {e}", path.display())))?,
        );
        let export = cxf::parse(&json)?;
        let (mut summary, created) = self.import_items(&export.items, export.skipped, vault_id)?;
        for passkey in &export.passkeys {
            let line = u64::try_from(passkey.item + 1).unwrap_or(u64::MAX);
            // Items that were already here keep what they have.
            let Some(Some(id)) = created.get(passkey.item) else {
                continue;
            };
            if let Err(e) = self.import_passkey(id, passkey) {
                summary
                    .skipped
                    .push((line, format!("passkey for {}: {e}", passkey.rp_id)));
            }
        }
        summary
            .skipped
            .extend(export.warnings.into_iter().map(|w| (0, w)));
        Ok(summary)
    }

    /// Imports a Bitwarden JSON export (unencrypted) into `vault_id`.
    pub fn import_bitwarden(
        &mut self,
        path: &Path,
        vault_id: Option<&str>,
    ) -> Result<ImportSummary> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| AppError::Io(format!("cannot open {}: {e}", path.display())))?;
        let export = bitwarden::parse(&json)?;
        Ok(self
            .import_items(&export.items, export.skipped, vault_id)?
            .0)
    }

    /// Imports a `KeePass` / `KeePassXC` database (`.kdbx`), opened with its
    /// own password and/or key file.
    pub fn import_kdbx(
        &mut self,
        path: &Path,
        password: &SecretString,
        key_file: Option<&Path>,
        vault_id: Option<&str>,
    ) -> Result<ImportSummary> {
        let open = |p: &Path| {
            File::open(p).map_err(|e| AppError::Io(format!("cannot open {}: {e}", p.display())))
        };
        let mut file = open(path)?;
        let mut key = key_file.map(open).transpose()?;
        let export = kdbx::read(
            &mut file,
            password.expose(),
            key.as_mut().map(|f| f as &mut dyn std::io::Read),
        )?;
        Ok(self
            .import_items(&export.items, export.skipped, vault_id)?
            .0)
    }

    /// Creates `items` in `vault_id`, skipping those already there. Also
    /// returns the id each item got (`None` when it was not created).
    fn import_items(
        &mut self,
        items: &[ImportedItem],
        skipped: usize,
        vault_id: Option<&str>,
    ) -> Result<(ImportSummary, Vec<Option<String>>)> {
        let vault = self.vault_or_default(vault_id)?.to_string();
        let (existing, mut known_logins) = {
            let session = self.session()?;
            let items = session.items();
            let existing: Vec<(String, String, Option<String>)> = items
                .iter()
                .map(|s| {
                    (
                        s.kind.as_str().to_owned(),
                        s.overview.title.clone(),
                        s.overview.username.clone(),
                    )
                })
                .collect();
            let logins: std::collections::HashSet<(String, String)> = items
                .iter()
                .filter(|s| s.kind == ItemKind::Login)
                .flat_map(|s| login_keys(&s.overview.urls, s.overview.username.as_deref()))
                .collect();
            (existing, logins)
        };
        let mut summary = ImportSummary::default();
        let mut created_ids = vec![None; items.len()];
        for (index, item) in items.iter().enumerate() {
            let input = to_input(item, &vault);
            let key = (
                item.kind.as_str().to_owned(),
                input.title.clone(),
                item.username.clone(),
            );
            // A login already stored for the same site and username (whoever imported it, under
            // whatever title) or repeated inside the file is not added again, as in a CSV import.
            let keys = if item.kind == ItemKind::Login {
                login_keys(&item.urls, item.username.as_deref())
            } else {
                Vec::new()
            };
            if existing.contains(&key) || keys.iter().any(|k| known_logins.contains(k)) {
                summary.identical += 1;
                continue;
            }
            known_logins.extend(keys);
            match self.create_item(&input) {
                Ok(created) => {
                    summary.imported += 1;
                    if item.archived {
                        self.set_archived(&created.summary.id, true)?;
                    }
                    created_ids[index] = Some(created.summary.id);
                }
                Err(e) => summary
                    .skipped
                    .push((u64::try_from(index + 1).unwrap_or(u64::MAX), e.to_string())),
            }
        }
        summary
            .skipped
            .extend((0..skipped).map(|_| (0, "unknown categories are not imported".to_owned())));
        Ok((summary, created_ids))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::service::tests::with_account;

    const EXPORT: &str = r#"{"accounts":[{"vaults":[{"items":[
      {"item":{"categoryUuid":"001","favIndex":1,"overview":{"title":"GitHub","url":"https://github.com"},
        "details":{"loginFields":[{"designation":"username","value":"octo"},{"designation":"password","value":"pw"}],
          "sections":[{"fields":[{"title":"one-time password","value":{"totp":"not a secret!"}}]}]}}},
      {"item":{"categoryUuid":"002","state":"archived","overview":{"title":"Visa"},
        "details":{"sections":[{"title":"","fields":[{"title":"number","id":"ccnum","value":{"creditCardNumber":"4111"}}]},
          {"title":"Contact Information","fields":[{"title":"phone","value":{"phone":"555"}}]}]}}},
      {"item":{"categoryUuid":"006","overview":{"title":"Scan"},
        "details":{"documentAttributes":{"fileName":"scan.pdf","documentId":"doc1","decryptedSize":4}}}},
      {"item":{"categoryUuid":"999","overview":{"title":"Unknown"}}}
    ]}]}]}"#;

    fn file(dir: &tempfile::TempDir) -> std::path::PathBuf {
        let path = dir.path().join("export.1pux");
        let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
        writer
            .start_file("export.data", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(EXPORT.as_bytes()).unwrap();
        writer
            .start_file(
                "files/doc1__scan.pdf",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"%PDF").unwrap();
        writer.finish().unwrap();
        path
    }

    #[test]
    fn imports_items_with_sections_archive_and_skips_repeats() {
        let mut fx = with_account();
        let dir = tempfile::tempdir().unwrap();
        let path = file(&dir);
        let summary = fx.app.import_1pux(&path, None).unwrap();
        assert_eq!(
            (summary.imported, summary.identical, summary.skipped.len()),
            (3, 0, 1)
        );
        let scan_id = fx.app.items("Scan", None).unwrap().remove(0).id;
        let scan = fx.app.item(&scan_id).unwrap();
        assert_eq!(
            (scan.summary.kind.as_str(), scan.attachments.len()),
            ("document", 1)
        );
        let file = &scan.attachments[0];
        assert_eq!(
            (file.name.as_str(), file.content_type.as_str()),
            ("scan.pdf", "application/pdf")
        );
        assert_eq!(
            fx.app
                .attachment_bytes(&scan_id, &file.id)
                .unwrap()
                .as_slice(),
            b"%PDF"
        );
        let login = fx.app.items("GitHub", None).unwrap().remove(0);
        assert!(login.favorite);
        let item = fx.app.item(&login.id).unwrap();
        assert!(!item.has_totp, "an unreadable code is kept as a field");
        assert_eq!(item.fields[0].label, "one-time password");
        let visa = fx.app.archived_items("").unwrap().remove(0);
        let visa = fx.app.item(&visa.id).unwrap();
        assert_eq!(
            visa.fields
                .iter()
                .find(|f| f.label == "phone")
                .unwrap()
                .section,
            "Contact Information"
        );
        assert!(
            visa.fields
                .iter()
                .find(|f| f.label == "number")
                .unwrap()
                .concealed
        );
        let again = fx.app.import_1pux(&path, None).unwrap();
        assert_eq!((again.imported, again.identical), (0, 3));
        let scan = fx.app.items("Scan", None).unwrap().remove(0);
        assert_eq!(
            fx.app.item(&scan.id).unwrap().attachments.len(),
            1,
            "not attached twice"
        );
        assert!(
            fx.app
                .import_1pux(&dir.path().join("missing.1pux"), None)
                .is_err()
        );
    }
}

#[cfg(test)]
mod login_dedupe_tests {
    use super::*;
    use crate::item_input::ItemInput;
    use crate::service::tests::with_account;

    #[test]
    fn a_login_already_stored_for_the_same_site_and_user_is_not_added_again() {
        let mut fx = with_account();
        fx.app
            .create_item(&ItemInput {
                title: "Apple title".into(),
                urls: vec!["https://a.com/login".into()],
                username: Some("Ann".into()),
                ..ItemInput::default()
            })
            .unwrap();
        let item = |title: &str, url: &str| ImportedItem {
            kind: ItemKind::Login,
            title: title.into(),
            username: Some("ann".into()),
            urls: vec![url.into()],
            password: None,
            notes: None,
            totp: None,
            tags: vec![],
            favorite: false,
            archived: false,
            fields: vec![],
        };
        let items = [
            item("a.com", "https://a.com"),
            item("b", "https://b.com"),
            item("b2", "https://b.com/x"),
        ];
        let (summary, _) = fx.app.import_items(&items, 0, None).unwrap();
        assert_eq!((summary.imported, summary.identical), (1, 2));
    }
}
