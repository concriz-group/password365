//! `KeePass` / `KeePassXC` databases (`.kdbx`, opened with their master
//! password): every entry with its folder path as a tag, its extra fields
//! (protected ones stay hidden) and its one-time password. The recycle bin is
//! left out. Key files and hardware keys are not supported here.

use keepass::db::{GroupRef, Value};
use std::io::Read as _;

use keepass::{Database, DatabaseKey};
use password365_model::ItemKind;

pub use crate::item::{ImportedField, ImportedItem};
use crate::{ImportError, Result};

/// Largest database file read.
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// The standard fields; anything else is an extra field.
const STANDARD: [&str; 5] = ["Title", "UserName", "Password", "URL", "Notes"];
/// Where `KeePassXC` (`otp`) and the `KeePass` TOTP plugin keep one-time passwords.
const OTP: [&str; 2] = ["otp", "TimeOtp-Secret-Base32"];

/// What a database held.
#[derive(Debug, Default)]
pub struct Kdbx {
    /// Entries that can be imported.
    pub items: Vec<ImportedItem>,
    /// Entries skipped (empty).
    pub skipped: usize,
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

fn walk(
    group: &GroupRef<'_>,
    path: &[String],
    recycle_bin: Option<keepass::db::GroupId>,
    out: &mut Kdbx,
) {
    for entry in group.entries() {
        let username = non_empty(entry.get_username());
        let password = entry
            .get_password()
            .filter(|p| !p.is_empty())
            .map(str::to_owned);
        let urls: Vec<String> = non_empty(entry.get_url()).into_iter().collect();
        let notes = non_empty(entry.get("Notes"));
        let totp = OTP.iter().find_map(|k| non_empty(entry.get(k)));
        let mut fields: Vec<ImportedField> = entry
            .fields
            .iter()
            .filter(|(key, _)| !STANDARD.contains(&key.as_str()) && !OTP.contains(&key.as_str()))
            .filter_map(|(key, value)| {
                let text = non_empty(entry.get(key))?;
                Some(ImportedField {
                    section: "More".into(),
                    label: key.clone(),
                    value: text,
                    concealed: matches!(value, Value::Protected(_)),
                    input: "text",
                })
            })
            .collect();
        fields.sort_by(|a, b| a.label.cmp(&b.label));
        let login = username.is_some() || password.is_some() || !urls.is_empty();
        let item = ImportedItem {
            kind: if login {
                ItemKind::Login
            } else {
                ItemKind::SecureNote
            },
            title: non_empty(entry.get_title()).unwrap_or_default(),
            username,
            urls,
            password,
            notes,
            totp,
            tags: path
                .iter()
                .cloned()
                .chain(entry.tags.iter().cloned())
                .collect(),
            favorite: false,
            archived: false,
            fields,
        };
        if login || item.notes.is_some() || !item.fields.is_empty() {
            out.items.push(item);
        } else {
            out.skipped += 1;
        }
    }
    for child in group.groups() {
        if Some(child.id()) == recycle_bin {
            continue;
        }
        let mut below = path.to_vec();
        below.push(child.name.clone());
        walk(&child, &below, recycle_bin, out);
    }
}

/// Opens a `.kdbx` with its master password and/or key file and reads every
/// entry. An empty password with a key file means "no password" (`KeePass`
/// tells the two apart).
pub fn read(
    source: &mut dyn std::io::Read,
    password: &str,
    key_file: Option<&mut dyn std::io::Read>,
) -> Result<Kdbx> {
    let mut key = DatabaseKey::new();
    if !password.is_empty() || key_file.is_none() {
        key = key.with_password(password);
    }
    if let Some(file) = key_file {
        key = key
            .with_keyfile(file)
            .map_err(|e| ImportError::Format(format!("cannot read the key file: {e}")))?;
    }
    let mut data = Vec::new();
    source
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|e| ImportError::Format(format!("cannot read the KeePass database: {e}")))?;
    if data.len() as u64 > MAX_FILE_BYTES {
        return Err(ImportError::Format(
            "this KeePass database is too large".into(),
        ));
    }
    crate::kdbx_limits::check(&data)?;
    let db = Database::parse(&data, key)
        .map_err(|e| ImportError::Format(format!("cannot open the KeePass database: {e}")))?;
    let mut out = Kdbx::default();
    let recycle_bin = db.recycle_bin().map(|g| g.id());
    walk(&db.root(), &[], recycle_bin, &mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use keepass::db::Value;

    use super::*;

    fn database() -> Vec<u8> {
        let mut db = Database::new();
        let mut root = db.root_mut();
        root.add_entry().edit(|e| {
            e.set_unprotected("Title", "GitHub");
            e.set_unprotected("UserName", "octo");
            e.set_protected("Password", "pw");
            e.set_unprotected("URL", "https://github.com");
            e.set_unprotected("otp", "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP");
            e.set_protected("Recovery code", "abc-123");
            e.set_unprotected("Plan", "Pro");
        });
        let mut bank = root.add_group();
        bank.edit(|g| g.name = "Bank".into());
        bank.add_entry().edit(|e| {
            e.set_unprotected("Title", "Safe");
            e.set("Notes", Value::Unprotected("combination 1-2-3".into()));
        });
        bank.add_entry()
            .edit(|e| e.set_unprotected("Title", "Empty"));
        let mut buffer = Vec::new();
        db.save(&mut buffer, DatabaseKey::new().with_password("demo"))
            .unwrap();
        buffer
    }

    #[test]
    fn opens_databases_protected_by_a_key_file() {
        let key_file = b"<?xml version=\"1.0\"?><KeyFile><Meta><Version>1.0</Version></Meta><Key><Data>c2VjcmV0IGtleSBmaWxlIGRhdGEgMzIgYnl0ZXMhIQ==</Data></Key></KeyFile>";
        let mut db = Database::new();
        db.root_mut().add_entry().edit(|e| {
            e.set_unprotected("Title", "Vault");
            e.set_protected("Password", "x");
        });
        let mut only_file = Vec::new();
        db.save(
            &mut only_file,
            DatabaseKey::new()
                .with_keyfile(&mut key_file.as_slice())
                .unwrap(),
        )
        .unwrap();
        let mut both = Vec::new();
        db.save(
            &mut both,
            DatabaseKey::new()
                .with_password("pw")
                .with_keyfile(&mut key_file.as_slice())
                .unwrap(),
        )
        .unwrap();

        let out = read(
            &mut only_file.as_slice(),
            "",
            Some(&mut key_file.as_slice()),
        )
        .unwrap();
        assert_eq!(out.items[0].title, "Vault");
        assert!(read(&mut both.as_slice(), "pw", Some(&mut key_file.as_slice())).is_ok());
        assert!(
            read(&mut both.as_slice(), "pw", None).is_err(),
            "the key file is needed"
        );
        assert!(read(&mut both.as_slice(), "pw", Some(&mut b"other".as_slice())).is_err());
    }

    #[test]
    fn reads_entries_with_folders_fields_and_codes() {
        let bytes = database();
        let out = read(&mut bytes.as_slice(), "demo", None).unwrap();
        assert_eq!(out.skipped, 1);
        let github = out.items.iter().find(|i| i.title == "GitHub").unwrap();
        assert_eq!(
            (github.username.as_deref(), github.password.as_deref()),
            (Some("octo"), Some("pw"))
        );
        assert!(github.totp.as_deref().unwrap().starts_with("otpauth://"));
        let labels: Vec<_> = github
            .fields
            .iter()
            .map(|f| (f.label.as_str(), f.concealed))
            .collect();
        assert_eq!(labels, [("Plan", false), ("Recovery code", true)]);
        let safe = out.items.iter().find(|i| i.title == "Safe").unwrap();
        assert_eq!(safe.kind, ItemKind::SecureNote);
        assert_eq!(safe.tags, ["Bank"]);
    }

    #[test]
    fn a_wrong_password_is_refused() {
        let bytes = database();
        assert!(
            read(&mut bytes.as_slice(), "nope", None)
                .unwrap_err()
                .to_string()
                .contains("KeePass")
        );
        assert!(read(&mut b"not a database".as_slice(), "x", None).is_err());
    }
}
