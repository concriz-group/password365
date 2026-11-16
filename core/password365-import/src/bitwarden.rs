//! Bitwarden's JSON export (unencrypted): logins, cards, identities, secure
//! notes and SSH keys, with custom fields, folders (as tags) and favorites.

use serde::Deserialize;
use serde_json::Value;

pub use crate::item::{ImportedField, ImportedItem};
use crate::{ImportError, Result};
use password365_model::ItemKind;

/// What a file held.
#[derive(Debug, Default)]
pub struct Bitwarden {
    /// Items that can be imported.
    pub items: Vec<ImportedItem>,
    /// Items skipped (unknown types or empty).
    pub skipped: usize,
}

#[derive(Deserialize)]
struct Export {
    #[serde(default)]
    encrypted: bool,
    #[serde(default)]
    folders: Vec<Folder>,
    #[serde(default)]
    items: Vec<Value>,
}

#[derive(Deserialize)]
struct Folder {
    id: String,
    name: String,
}

fn text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn field(
    label: &str,
    value: Option<String>,
    concealed: bool,
    input: &'static str,
) -> Option<ImportedField> {
    value.map(|value| ImportedField {
        section: String::new(),
        label: label.to_owned(),
        value,
        concealed,
        input,
    })
}

fn card_fields(card: &Value) -> Vec<ImportedField> {
    let get = |k: &str| text(card.get(k));
    let expiry = match (get("expYear"), get("expMonth")) {
        (Some(year), Some(month)) if year.len() == 4 => Some(format!("{year}-{month:0>2}")),
        _ => None,
    };
    [
        field("cardholder name", get("cardholderName"), false, "text"),
        field("type", get("brand"), false, "text"),
        field("number", get("number"), true, "text"),
        field("verification number", get("code"), true, "text"),
        field("expiry date", expiry, false, "month_year"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn identity_fields(identity: &Value) -> Vec<ImportedField> {
    let get = |k: &str| text(identity.get(k));
    let street: Vec<String> = ["address1", "address2", "address3"]
        .iter()
        .filter_map(|k| get(k))
        .collect();
    let section = |name: &str, f: Option<ImportedField>| {
        f.map(|f| ImportedField {
            section: name.to_owned(),
            ..f
        })
    };
    [
        section(
            "Identification",
            field("first name", get("firstName"), false, "text"),
        ),
        section(
            "Identification",
            field("initial", get("middleName"), false, "text"),
        ),
        section(
            "Identification",
            field("last name", get("lastName"), false, "text"),
        ),
        section(
            "Identification",
            field("company", get("company"), false, "text"),
        ),
        section(
            "Address",
            field(
                "address",
                (!street.is_empty()).then(|| street.join("\n")),
                false,
                "multiline",
            ),
        ),
        section("Address", field("city", get("city"), false, "text")),
        section("Address", field("state", get("state"), false, "text")),
        section("Address", field("zip", get("postalCode"), false, "text")),
        section("Address", field("country", get("country"), false, "text")),
        section("Address", field("phone", get("phone"), false, "phone")),
        section(
            "Internet Details",
            field("email", get("email"), false, "email"),
        ),
        section(
            "Internet Details",
            field("username", get("username"), false, "text"),
        ),
        section(
            "Documents",
            field("social security number", get("ssn"), true, "text"),
        ),
        section(
            "Documents",
            field("passport number", get("passportNumber"), false, "text"),
        ),
        section(
            "Documents",
            field("license number", get("licenseNumber"), false, "text"),
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn item_of(value: &Value, folders: &[Folder]) -> Option<ImportedItem> {
    let kind = match value.get("type")?.as_u64()? {
        1 => ItemKind::Login,
        2 => ItemKind::SecureNote,
        3 => ItemKind::CreditCard,
        4 => ItemKind::Identity,
        5 => ItemKind::SshKey,
        _ => return None,
    };
    let login = value.get("login").cloned().unwrap_or(Value::Null);
    let folder = value
        .get("folderId")
        .and_then(Value::as_str)
        .and_then(|id| folders.iter().find(|f| f.id == id));
    let mut item = ImportedItem {
        kind,
        title: text(value.get("name")).unwrap_or_default(),
        username: text(login.get("username")),
        urls: login
            .get("uris")
            .and_then(Value::as_array)
            .map(|u| u.iter().filter_map(|u| text(u.get("uri"))).collect())
            .unwrap_or_default(),
        password: text(login.get("password")),
        notes: text(value.get("notes")),
        totp: text(login.get("totp")),
        tags: folder.map(|f| vec![f.name.clone()]).unwrap_or_default(),
        favorite: value
            .get("favorite")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        archived: false,
        fields: Vec::new(),
    };
    item.fields = match kind {
        ItemKind::CreditCard => value.get("card").map(card_fields).unwrap_or_default(),
        ItemKind::Identity => value
            .get("identity")
            .map(identity_fields)
            .unwrap_or_default(),
        ItemKind::SshKey => {
            let key = value.get("sshKey").cloned().unwrap_or(Value::Null);
            [
                field(
                    "private key",
                    text(key.get("privateKey")),
                    true,
                    "multiline",
                ),
                field("public key", text(key.get("publicKey")), false, "multiline"),
                field(
                    "fingerprint",
                    text(key.get("keyFingerprint")),
                    false,
                    "text",
                ),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        _ => Vec::new(),
    };
    for custom in value
        .get("fields")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let label = text(custom.get("name")).unwrap_or_else(|| "field".into());
        let hidden = custom.get("type").and_then(Value::as_u64) == Some(1);
        if let Some(value) = text(custom.get("value"))
            && !item
                .fields
                .iter()
                .any(|f| f.label.eq_ignore_ascii_case(&label))
        {
            item.fields.push(ImportedField {
                section: "More".into(),
                label,
                value,
                concealed: hidden,
                input: "text",
            });
        }
    }
    (!item.title.is_empty() || item.password.is_some() || !item.fields.is_empty()).then_some(item)
}

/// Reads a Bitwarden JSON export.
pub fn parse(json: &str) -> Result<Bitwarden> {
    let export: Export = serde_json::from_str(json)
        .map_err(|e| ImportError::Format(format!("not a Bitwarden JSON export: {e}")))?;
    if export.encrypted {
        return Err(ImportError::Format(
            "this Bitwarden export is encrypted: export again choosing the .json format (not “.json (Encrypted)”)".into(),
        ));
    }
    let mut out = Bitwarden::default();
    for value in &export.items {
        match item_of(value, &export.folders) {
            Some(item) => out.items.push(item),
            None => out.skipped += 1,
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPORT: &str = r#"{"encrypted":false,"folders":[{"id":"f1","name":"Bank"}],"items":[
      {"type":1,"name":"GitHub","favorite":true,"folderId":"f1","notes":"n",
       "login":{"username":"octo","password":"pw","totp":"JBSWY3DPEHPK3PXP","uris":[{"uri":"https://github.com"}]},
       "fields":[{"name":"PIN","value":"1234","type":1},{"name":"empty","value":null,"type":0}]},
      {"type":3,"name":"Visa","card":{"cardholderName":"Ana","brand":"Visa","number":"4111","expMonth":"4","expYear":"2029","code":"123"}},
      {"type":4,"name":"Me","identity":{"firstName":"Ana","address1":"Rua A, 10","address2":"Apto 3","postalCode":"01000-000","ssn":"000-00-0000"}},
      {"type":2,"name":"Wi-Fi","notes":"key"},
      {"type":5,"name":"Deploy","sshKey":{"privateKey":"-----BEGIN OPENSSH PRIVATE KEY-----","publicKey":"ssh-ed25519 AAA","keyFingerprint":"SHA256:x"}},
      {"type":9,"name":"???"}
    ]}"#;

    #[test]
    fn reads_every_type_with_folders_and_custom_fields() {
        let out = parse(EXPORT).unwrap();
        assert_eq!(out.skipped, 1);
        let [login, card, identity, note, ssh] = out.items.as_slice() else {
            panic!("{:?}", out.items)
        };
        assert_eq!(
            (login.username.as_deref(), login.password.as_deref()),
            (Some("octo"), Some("pw"))
        );
        assert_eq!(login.urls, ["https://github.com"]);
        assert!(login.favorite && login.tags == ["Bank"]);
        assert_eq!(login.fields.len(), 1);
        assert!(login.fields[0].concealed && login.fields[0].section == "More");
        let get = |item: &ImportedItem, label: &str| {
            item.fields
                .iter()
                .find(|f| f.label == label)
                .map(|f| f.value.clone())
        };
        assert_eq!(get(card, "expiry date").as_deref(), Some("2029-04"));
        assert_eq!(
            get(identity, "address").as_deref(),
            Some("Rua A, 10\nApto 3")
        );
        assert_eq!(note.kind, ItemKind::SecureNote);
        assert_eq!(get(ssh, "fingerprint").as_deref(), Some("SHA256:x"));
    }

    #[test]
    fn refuses_encrypted_and_other_files() {
        assert!(
            parse(r#"{"encrypted":true,"items":[]}"#)
                .unwrap_err()
                .to_string()
                .contains("encrypted")
        );
        assert!(parse("[1,2]").is_err());
    }
}
