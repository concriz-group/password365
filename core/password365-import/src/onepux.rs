//! 1Password's full export (`.1pux`): a zip with `export.data`, the JSON of
//! every account, vault and item. Unlike its CSV it keeps identities, cards,
//! notes, sections, tags, favorites, the archive and attached files (kept in
//! the zip as `files/<documentId>__<fileName>`).

use std::io::{Read, Seek};

use password365_model::ItemKind;
use serde::Deserialize;
use serde_json::Value;

pub use crate::item::{ImportedField, ImportedItem};
use crate::{ImportError, Result};

/// What a file held.
#[derive(Debug, Default)]
pub struct OnePux {
    /// Items that can be imported.
    pub items: Vec<ImportedItem>,
    /// Items skipped (unknown categories, empty).
    pub skipped: usize,
    /// Files attached to the items.
    pub files: Vec<OnePuxFile>,
}

/// A file attached to an item of the export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnePuxFile {
    /// Index of its item in [`OnePux::items`].
    pub item: usize,
    /// 1Password's id for it (its name in the zip starts with it).
    pub document_id: String,
    /// File name.
    pub name: String,
    /// The bytes (empty until read from the zip; `parse` alone leaves them empty).
    pub bytes: Vec<u8>,
}

/// The files an item refers to: a Document's own, then file fields.
fn file_refs(details: &Value) -> Vec<(String, String)> {
    let reference = |v: &Value| {
        let id = v.get("documentId").and_then(text)?;
        let name = v
            .get("fileName")
            .and_then(text)
            .unwrap_or_else(|| "file".into());
        Some((id, name))
    };
    let mut out: Vec<(String, String)> = details
        .get("documentAttributes")
        .and_then(reference)
        .into_iter()
        .collect();
    for field in details
        .get("sections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.get("fields").and_then(Value::as_array))
        .flatten()
    {
        if let Some(found) = field
            .get("value")
            .and_then(|v| v.get("file"))
            .and_then(reference)
        {
            out.push(found);
        }
    }
    out
}

#[derive(Deserialize)]
struct Export {
    #[serde(default)]
    accounts: Vec<Account>,
}

#[derive(Deserialize)]
struct Account {
    #[serde(default)]
    vaults: Vec<Vault>,
}

#[derive(Deserialize)]
struct Vault {
    #[serde(default)]
    items: Vec<Value>,
}

/// 1Password's category ids.
fn kind(category: &str) -> Option<ItemKind> {
    Some(match category {
        "001" => ItemKind::Login,
        "002" => ItemKind::CreditCard,
        "003" => ItemKind::SecureNote,
        "004" => ItemKind::Identity,
        "005" => ItemKind::Password,
        "006" => ItemKind::Document,
        "100" => ItemKind::SoftwareLicense,
        "101" => ItemKind::BankAccount,
        "102" => ItemKind::Database,
        "103" => ItemKind::DriverLicense,
        "104" => ItemKind::OutdoorLicense,
        "105" => ItemKind::Membership,
        "106" => ItemKind::Passport,
        "107" => ItemKind::Rewards,
        "108" => ItemKind::SocialSecurityNumber,
        "109" => ItemKind::WirelessRouter,
        "110" => ItemKind::Server,
        "111" => ItemKind::Email,
        "112" => ItemKind::ApiCredential,
        "113" => ItemKind::MedicalRecord,
        "114" => ItemKind::SshKey,
        "115" => ItemKind::CryptoWallet,
        _ => return None,
    })
}

/// Template labels for 1Password field ids whose title the export leaves empty.
fn label_for_id(id: &str) -> Option<&'static str> {
    Some(match id {
        "cardholder" => "cardholder name",
        "type" => "type",
        "ccnum" => "number",
        "cvv" => "verification number",
        "expiry" => "expiry date",
        "validFrom" => "valid from",
        "pin" => "PIN",
        "bank" => "issuing bank",
        "firstname" => "first name",
        "initial" => "initial",
        "lastname" => "last name",
        "birthdate" => "birth date",
        "occupation" => "occupation",
        "company" => "company",
        "jobtitle" => "job title",
        "address" => "address",
        "defphone" => "phone",
        "email" => "email",
        "website" => "website",
        "private_key" => "private key",
        "public_key" => "public key",
        "fingerprint" => "fingerprint",
        "key_type" => "key type",
        _ => return None,
    })
}

fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `2026-11-14` from a Unix time in seconds.
fn date(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The fields one 1Password field becomes (an address becomes several).
fn fields_of(section: &str, field: &Value) -> Vec<ImportedField> {
    let title = field
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let id = field.get("id").and_then(Value::as_str).unwrap_or_default();
    let label = if title.is_empty() {
        label_for_id(id).map_or_else(|| id.to_owned(), str::to_owned)
    } else {
        title
    };
    let Some(Value::Object(value)) = field.get("value") else {
        return Vec::new();
    };
    let Some((kind, raw)) = value.iter().next() else {
        return Vec::new();
    };
    let one = |value: String, concealed: bool, input: &'static str| {
        vec![ImportedField {
            section: section.to_owned(),
            label: label.clone(),
            value,
            concealed,
            input,
        }]
    };
    match kind.as_str() {
        "concealed" | "creditCardNumber" => {
            text(raw).map_or_else(Vec::new, |v| one(v, true, "text"))
        }
        "totp" => text(raw).map_or_else(Vec::new, |v| one(v, false, "totp")),
        "email" => raw
            .get("email_address")
            .and_then(text)
            .or_else(|| text(raw))
            .map_or_else(Vec::new, |v| one(v, false, "email")),
        "phone" => text(raw).map_or_else(Vec::new, |v| one(v, false, "phone")),
        "url" => text(raw).map_or_else(Vec::new, |v| one(v, false, "url")),
        "date" => raw
            .as_i64()
            .map_or_else(Vec::new, |s| one(date(s), false, "date")),
        "monthYear" => raw
            .as_i64()
            .filter(|n| *n > 100_000)
            .map_or_else(Vec::new, |n| {
                one(
                    format!("{:04}-{:02}", n / 100, n % 100),
                    false,
                    "month_year",
                )
            }),
        "address" => {
            let part = |k: &str| raw.get(k).and_then(text);
            let mut out = Vec::new();
            for (key, label) in [
                ("street", "address"),
                ("city", "city"),
                ("state", "state"),
                ("zip", "zip"),
                ("country", "country"),
            ] {
                if let Some(value) = part(key) {
                    out.push(ImportedField {
                        section: section.to_owned(),
                        label: label.into(),
                        value,
                        concealed: false,
                        input: "text",
                    });
                }
            }
            out
        }
        _ => text(raw).map_or_else(Vec::new, |v| {
            let input = if v.contains('\n') {
                "multiline"
            } else {
                "text"
            };
            one(v, false, input)
        }),
    }
}

fn item_of(value: &Value) -> Option<ImportedItem> {
    let kind = kind(value.get("categoryUuid")?.as_str()?)?;
    let overview = value.get("overview")?;
    let details = value.get("details").cloned().unwrap_or(Value::Null);
    let title = overview.get("title").and_then(text).unwrap_or_default();
    let mut item = ImportedItem {
        kind,
        title,
        username: None,
        urls: Vec::new(),
        password: None,
        notes: details.get("notesPlain").and_then(text),
        totp: None,
        tags: overview
            .get("tags")
            .and_then(Value::as_array)
            .map(|t| t.iter().filter_map(text).collect())
            .unwrap_or_default(),
        favorite: value
            .get("favIndex")
            .and_then(Value::as_i64)
            .is_some_and(|i| i > 0),
        archived: value.get("state").and_then(Value::as_str) == Some("archived"),
        fields: Vec::new(),
    };
    if let Some(urls) = overview.get("urls").and_then(Value::as_array) {
        item.urls = urls
            .iter()
            .filter_map(|u| u.get("url").and_then(text))
            .collect();
    }
    if item.urls.is_empty()
        && let Some(url) = overview.get("url").and_then(text)
    {
        item.urls.push(url);
    }
    for field in details
        .get("loginFields")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let value = field.get("value").and_then(text);
        match field.get("designation").and_then(Value::as_str) {
            Some("username") => item.username = item.username.take().or(value),
            Some("password") => item.password = item.password.take().or(value),
            _ => {}
        }
    }
    if let Some(password) = details.get("password").and_then(text) {
        item.password.get_or_insert(password);
    }
    for section in details
        .get("sections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let heading = section
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();
        for field in section
            .get("fields")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for imported in fields_of(heading, field) {
                // A login's one-time password is the item's own.
                if matches!(kind, ItemKind::Login | ItemKind::Password)
                    && imported.input == "totp"
                    && item.totp.is_none()
                {
                    item.totp = Some(imported.value);
                } else if !item
                    .fields
                    .iter()
                    .any(|f| f.label.eq_ignore_ascii_case(&imported.label))
                {
                    item.fields.push(imported);
                }
            }
        }
    }
    (!item.title.is_empty() || item.password.is_some() || !item.fields.is_empty()).then_some(item)
}

/// Most bytes `export.data` may unpack to.
const MAX_EXPORT_DATA: u64 = 256 * 1024 * 1024;
/// Most bytes all attached files together may unpack to.
const MAX_FILES_TOTAL: u64 = 2 * 1024 * 1024 * 1024;

/// Reads at most `limit` bytes of `entry` into `out`; fails beyond (a zip
/// can claim small sizes and unpack to far more: never trust its header).
fn read_limited(entry: impl Read, limit: u64, out: &mut Vec<u8>, what: &str) -> Result<()> {
    let start = out.len();
    entry
        .take(limit + 1)
        .read_to_end(out)
        .map_err(|_| ImportError::Format(format!("{what} cannot be read")))?;
    if (out.len() - start) as u64 > limit {
        return Err(ImportError::Format(format!("{what} is too large")));
    }
    Ok(())
}

/// Reads a `.1pux` file.
pub fn read<R: Read + Seek>(reader: R) -> Result<OnePux> {
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|_| ImportError::Format("not a 1Password export (.1pux)".into()))?;
    let mut data = Vec::new();
    read_limited(
        archive
            .by_name("export.data")
            .map_err(|_| ImportError::Format("export.data is missing".into()))?,
        MAX_EXPORT_DATA,
        &mut data,
        "export.data",
    )?;
    let json = String::from_utf8(data)
        .map_err(|_| ImportError::Format("export.data cannot be read".into()))?;
    let mut export = parse(&json)?;
    let names: Vec<String> = archive
        .file_names()
        .filter_map(|n| n.ok().map(std::borrow::Cow::into_owned))
        .collect();
    let mut total = 0;
    for file in &mut export.files {
        let prefix = format!("files/{}", file.document_id);
        let Some(entry) = names.iter().find(|n| n.starts_with(&prefix)) else {
            continue;
        };
        let entry = archive
            .by_name(entry)
            .map_err(|_| ImportError::Format(format!("{} cannot be read", file.name)))?;
        let limit = password365_model::attachment::MAX_ATTACHMENT_BYTES
            .min(MAX_FILES_TOTAL.saturating_sub(total));
        read_limited(entry, limit, &mut file.bytes, &file.name)?;
        total += file.bytes.len() as u64;
    }
    // A file the zip does not have cannot be attached.
    export.files.retain(|f| !f.bytes.is_empty());
    Ok(export)
}

/// Reads `export.data` (the JSON inside a `.1pux`).
pub fn parse(json: &str) -> Result<OnePux> {
    let export: Export =
        serde_json::from_str(json).map_err(|e| ImportError::Format(format!("export.data: {e}")))?;
    let mut out = OnePux::default();
    for value in export
        .accounts
        .iter()
        .flat_map(|a| &a.vaults)
        .flat_map(|v| &v.items)
    {
        let value = value.get("item").unwrap_or(value);
        match item_of(value) {
            Some(item) => {
                let details = value.get("details").unwrap_or(&Value::Null);
                for (document_id, name) in file_refs(details) {
                    out.files.push(OnePuxFile {
                        item: out.items.len(),
                        document_id,
                        name,
                        bytes: Vec::new(),
                    });
                }
                out.items.push(item);
            }
            None => out.skipped += 1,
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    #[test]
    fn unpacked_parts_are_capped_whatever_the_zip_claims() {
        let mut out = Vec::new();
        assert!(read_limited(&[0u8; 10][..], 10, &mut out, "x").is_ok());
        let mut out = Vec::new();
        let err = read_limited(std::io::repeat(0), 1024, &mut out, "export.data").unwrap_err();
        assert!(err.to_string().contains("too large"));
        assert!(out.len() <= 1025, "stops reading right after the limit");
    }

    const EXPORT: &str = r#"{"accounts":[{"vaults":[{"items":[
      {"item":{"categoryUuid":"001","favIndex":1,"state":"active",
        "overview":{"title":"GitHub","url":"https://github.com","tags":["dev"]},
        "details":{"loginFields":[{"designation":"username","value":"octo"},{"designation":"password","value":"pw"}],
          "notesPlain":"hello","sections":[{"title":"","fields":[{"title":"one-time password","id":"TOTP_x","value":{"totp":"otpauth://totp/x?secret=ABC"}}]}]}}},
      {"item":{"categoryUuid":"002","state":"archived","overview":{"title":"Visa"},
        "details":{"sections":[{"title":"","fields":[
          {"title":"","id":"cardholder","value":{"string":"Ana"}},
          {"title":"number","id":"ccnum","value":{"creditCardNumber":"4111111111111111"}},
          {"title":"","id":"expiry","value":{"monthYear":202904}}]},
          {"title":"Contact Information","fields":[{"title":"phone","id":"x","value":{"phone":"555"}}]}]}}},
      {"item":{"categoryUuid":"004","overview":{"title":"Me"},
        "details":{"sections":[{"title":"Identification","fields":[
          {"title":"first name","value":{"string":"Ana"}},{"title":"birth date","value":{"date":631152000}}]},
          {"title":"Address","fields":[{"title":"address","value":{"address":{"street":"Rua A, 10","city":"SP","zip":"01000-000","country":"br","state":""}}}]},
          {"title":"","fields":[{"title":"photo","value":{"file":{"fileName":"me.jpg","documentId":"doc2","decryptedSize":3}}}]}]}}},
      {"item":{"categoryUuid":"006","overview":{"title":"Scan"},
        "details":{"documentAttributes":{"fileName":"scan.pdf","documentId":"doc1","decryptedSize":4}}}},
      {"item":{"categoryUuid":"999","overview":{"title":"Unknown"}}}
    ]}]}]}"#;

    #[test]
    fn reads_logins_cards_identities_and_documents() {
        let out = parse(EXPORT).unwrap();
        assert_eq!(out.skipped, 1, "the unknown category");
        let [login, card, identity, document] = out.items.as_slice() else {
            panic!("{:?}", out.items)
        };
        assert_eq!(
            (
                login.kind,
                login.username.as_deref(),
                login.password.as_deref()
            ),
            (ItemKind::Login, Some("octo"), Some("pw"))
        );
        assert_eq!(login.urls, ["https://github.com"]);
        assert_eq!(login.totp.as_deref(), Some("otpauth://totp/x?secret=ABC"));
        assert!(
            login.favorite && !login.archived && login.tags == ["dev"] && login.fields.is_empty()
        );
        assert!(card.archived);
        let field = |item: &ImportedItem, label: &str| {
            item.fields
                .iter()
                .find(|f| f.label == label)
                .cloned()
                .unwrap()
        };
        assert_eq!(field(card, "cardholder name").value, "Ana");
        assert!(field(card, "number").concealed);
        assert_eq!(field(card, "expiry date").value, "2029-04");
        assert_eq!(field(card, "phone").section, "Contact Information");
        assert_eq!(field(identity, "birth date").value, "1990-01-01");
        assert_eq!(field(identity, "address").value, "Rua A, 10");
        assert_eq!(field(identity, "zip").section, "Address");
        assert!(
            !identity.fields.iter().any(|f| f.label == "state"),
            "empty parts dropped"
        );
        assert!(
            !identity.fields.iter().any(|f| f.label == "photo"),
            "a file is not a text field"
        );
        assert_eq!(
            (document.kind, document.title.as_str()),
            (ItemKind::Document, "Scan")
        );
        let refs: Vec<(usize, &str, &str)> = out
            .files
            .iter()
            .map(|f| (f.item, f.document_id.as_str(), f.name.as_str()))
            .collect();
        assert_eq!(refs, [(2, "doc2", "me.jpg"), (3, "doc1", "scan.pdf")]);
    }

    #[test]
    fn reads_the_zip_and_refuses_other_files() {
        let mut zipped = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut zipped);
            writer
                .start_file(
                    "export.data",
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(EXPORT.as_bytes()).unwrap();
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("files/doc1__scan.pdf", options).unwrap();
            writer.write_all(b"%PDF").unwrap();
            writer.finish().unwrap();
        }
        zipped.set_position(0);
        let export = read(zipped).unwrap();
        assert_eq!(export.items.len(), 4);
        let [file] = export.files.as_slice() else {
            panic!("only the file the zip holds: {:?}", export.files)
        };
        assert_eq!(
            (file.item, file.name.as_str(), file.bytes.as_slice()),
            (3, "scan.pdf", b"%PDF".as_slice())
        );
        assert!(read(Cursor::new(b"not a zip".to_vec())).is_err());
        assert!(parse("{").is_err());
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(1_794_614_400), "2026-11-14");
    }
}
