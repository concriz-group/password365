//! Reading the FIDO Alliance Credential Exchange Format (CXF 1.0): the JSON
//! `Header` other password managers export. Logins, passkeys, one-time
//! passwords, cards, notes and custom fields map directly; the other kinds
//! (addresses, documents, Wi-Fi, API keys, …) keep their values as fields of
//! the nearest category, so nothing readable is lost.

use password365_model::ItemKind;
use serde_json::{Map, Value};

use crate::item::{ImportedField, ImportedItem};
use crate::{ImportError, Result};

/// A passkey found in the file (only the app can store it).
#[derive(Clone, PartialEq, Eq)]
pub struct CxfPasskey {
    /// Index of its item in [`Cxf::items`].
    pub item: usize,
    /// Credential id (base64url).
    pub credential_id: String,
    /// Relying-party id.
    pub rp_id: String,
    /// Account name.
    pub user_name: String,
    /// The site's id for the account (base64url).
    pub user_handle: String,
    /// Private key, PKCS#8 DER.
    pub key: Vec<u8>,
}

impl std::fmt::Debug for CxfPasskey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CxfPasskey")
            .field("item", &self.item)
            .field("rp_id", &self.rp_id)
            .finish_non_exhaustive()
    }
}

/// What a file held.
#[derive(Debug, Default)]
pub struct Cxf {
    /// Items to create.
    pub items: Vec<ImportedItem>,
    /// Passkeys of those items.
    pub passkeys: Vec<CxfPasskey>,
    /// Items with nothing to import.
    pub skipped: usize,
    /// Parts that could not be imported, one line each (`"title: what"`).
    pub warnings: Vec<String>,
}

fn text(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

/// The `value` of an `EditableField`.
fn editable(v: Option<&Value>) -> Option<String> {
    v.and_then(|f| f.get("value")).and_then(text)
}

fn b64url(v: Option<&Value>) -> Option<Vec<u8>> {
    let s = v?.as_str()?.trim_end_matches('=');
    data_encoding::BASE64URL_NOPAD.decode(s.as_bytes()).ok()
}

/// `documentNumber` → `document number`.
fn words(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_uppercase() && !out.is_empty() {
            out.push(' ');
        }
        out.extend(c.to_lowercase());
    }
    out.replace(['-', '_'], " ")
}

fn input_of(field_type: &str) -> &'static str {
    match field_type {
        "date" => "date",
        "year-month" => "month_year",
        "email" => "email",
        _ => "text",
    }
}

/// One `EditableField` as a field of `section` (its own label, else `fallback`).
fn field_of(v: &Value, section: &str, fallback: impl FnOnce() -> String) -> Option<ImportedField> {
    let value = editable(Some(v))?;
    let field_type = v
        .get("fieldType")
        .and_then(Value::as_str)
        .unwrap_or("string");
    Some(ImportedField {
        section: section.to_owned(),
        label: v.get("label").and_then(text).unwrap_or_else(fallback),
        value,
        concealed: field_type == "concealed-string",
        input: input_of(field_type),
    })
}

/// Every `EditableField` member of a credential, as fields of `section`.
fn editable_fields(
    credential: &Map<String, Value>,
    section: &str,
    skip: &[&str],
) -> Vec<ImportedField> {
    credential
        .iter()
        .filter(|(name, _)| !skip.contains(&name.as_str()))
        .filter_map(|(name, v)| field_of(v, section, || words(name)))
        .collect()
}

/// A card's members under the card template's labels.
fn card_label(member: &str) -> Option<&'static str> {
    Some(match member {
        "fullName" => "cardholder name",
        "number" => "number",
        "cardType" => "type",
        "verificationNumber" => "verification number",
        "pin" => "PIN",
        "expiryDate" => "expiry date",
        "validFrom" => "valid from",
        _ => return None,
    })
}

/// The category a credential type suggests (most specific first wins).
fn kind_of(kind: &str) -> Option<(u8, ItemKind)> {
    Some(match kind {
        "basic-auth" | "passkey" => (0, ItemKind::Login),
        "credit-card" => (1, ItemKind::CreditCard),
        "wifi" => (2, ItemKind::WirelessRouter),
        "api-key" => (2, ItemKind::ApiCredential),
        "drivers-license" => (2, ItemKind::DriverLicense),
        "passport" => (2, ItemKind::Passport),
        "ssh-key" => (2, ItemKind::SshKey),
        "identity-document" | "address" | "person-name" => (3, ItemKind::Identity),
        "totp" => (4, ItemKind::Login),
        "generated-password" => (5, ItemKind::Password),
        _ => return None,
    })
}

/// `otpauth://` when the code is not the usual 6 digits / 30 s / SHA-1.
fn totp_value(c: &Map<String, Value>) -> Option<String> {
    let secret = c.get("secret").and_then(text)?;
    let algorithm = c
        .get("algorithm")
        .and_then(Value::as_str)
        .unwrap_or("sha1")
        .to_ascii_uppercase();
    let digits = c.get("digits").and_then(Value::as_u64).unwrap_or(6);
    let period = c.get("period").and_then(Value::as_u64).unwrap_or(30);
    if algorithm == "SHA1" && digits == 6 && period == 30 {
        return Some(secret);
    }
    Some(format!(
        "otpauth://totp/Imported?secret={secret}&algorithm={algorithm}&digits={digits}&period={period}"
    ))
}

struct Reader<'a> {
    item: ImportedItem,
    out: &'a mut Cxf,
    index: usize,
    notes: Vec<String>,
}

impl Reader<'_> {
    fn warn(&mut self, what: &str) {
        let title = if self.item.title.is_empty() {
            "(untitled)"
        } else {
            &self.item.title
        };
        self.out.warnings.push(format!("{title}: {what}"));
    }

    fn credential(&mut self, c: &Map<String, Value>) {
        let kind = c.get("type").and_then(Value::as_str).unwrap_or_default();
        match kind {
            "basic-auth" => {
                self.item.username = self
                    .item
                    .username
                    .take()
                    .or_else(|| editable(c.get("username")));
                self.item.password = self
                    .item
                    .password
                    .take()
                    .or_else(|| editable(c.get("password")));
            }
            "passkey" => self.passkey(c),
            "totp" if self.item.totp.is_none() => self.item.totp = totp_value(c),
            "note" => self.notes.extend(editable(c.get("content"))),
            "credit-card" => {
                for (member, v) in c {
                    if let (Some(label), Some(value)) = (card_label(member), editable(Some(v))) {
                        let field_type = v
                            .get("fieldType")
                            .and_then(Value::as_str)
                            .unwrap_or("string");
                        self.item.fields.push(ImportedField {
                            section: String::new(),
                            label: label.into(),
                            value,
                            concealed: matches!(
                                member.as_str(),
                                "number" | "verificationNumber" | "pin"
                            ),
                            input: input_of(field_type),
                        });
                    }
                }
            }
            "custom-fields" => {
                let section = c.get("label").and_then(text).unwrap_or_default();
                let fields = c
                    .get("fields")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten();
                self.item
                    .fields
                    .extend(fields.filter_map(|f| field_of(f, &section, || "field".into())));
            }
            "file" => {
                self.warn("attached files come only through a direct transfer, not in a file");
            }
            "ssh-key" => {
                self.warn("the SSH private key was not imported (add it from the key file)");
                self.item.fields.extend(editable_fields(c, "", &[]));
            }
            other => {
                let section = if kind_of(other).is_some() {
                    String::new()
                } else {
                    words(other)
                };
                self.item
                    .fields
                    .extend(editable_fields(c, &section, &["type"]));
            }
        }
    }

    fn passkey(&mut self, c: &Map<String, Value>) {
        let (Some(credential_id), Some(rp_id), Some(user_handle), Some(key)) = (
            c.get("credentialId").and_then(text),
            c.get("rpId").and_then(text),
            c.get("userHandle").and_then(text),
            b64url(c.get("key")),
        ) else {
            return self.warn("a passkey was incomplete");
        };
        if self.out.passkeys.iter().any(|p| p.item == self.index) {
            return self.warn("only one passkey per login is kept");
        }
        let user_name = c
            .get("username")
            .and_then(text)
            .or_else(|| self.item.username.clone())
            .unwrap_or_default();
        self.out.passkeys.push(CxfPasskey {
            item: self.index,
            credential_id,
            rp_id,
            user_name,
            user_handle,
            key,
        });
    }
}

fn item_of(value: &Value, out: &mut Cxf) {
    let credentials: Vec<&Map<String, Value>> = value
        .get("credentials")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .collect();
    let kind = credentials
        .iter()
        .filter_map(|c| c.get("type").and_then(Value::as_str).and_then(kind_of))
        .min_by_key(|(rank, _)| *rank)
        .map_or(ItemKind::SecureNote, |(_, kind)| kind);
    let item = ImportedItem {
        kind,
        title: value.get("title").and_then(text).unwrap_or_default(),
        username: None,
        urls: value
            .get("scope")
            .and_then(|s| s.get("urls"))
            .and_then(Value::as_array)
            .map(|u| u.iter().filter_map(text).collect())
            .unwrap_or_default(),
        password: None,
        notes: None,
        totp: None,
        tags: value
            .get("tags")
            .and_then(Value::as_array)
            .map(|t| t.iter().filter_map(text).collect())
            .unwrap_or_default(),
        favorite: value
            .get("favorite")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        archived: false,
        fields: Vec::new(),
    };
    let index = out.items.len();
    let mut reader = Reader {
        item,
        out,
        index,
        notes: Vec::new(),
    };
    for c in credentials {
        reader.credential(c);
    }
    let Reader {
        mut item,
        out,
        notes,
        ..
    } = reader;
    if !notes.is_empty() {
        item.notes = Some(notes.join("\n\n"));
    }
    let has_passkey = out.passkeys.iter().any(|p| p.item == index);
    let empty = item.username.is_none()
        && item.password.is_none()
        && item.notes.is_none()
        && item.totp.is_none()
        && item.fields.is_empty()
        && !has_passkey;
    if empty {
        out.skipped += 1;
    } else {
        out.items.push(item);
    }
}

/// Reads a CXF `Header` (JSON).
pub fn parse(json: &str) -> Result<Cxf> {
    let header: Value = serde_json::from_str(json)
        .map_err(|e| ImportError::Format(format!("not a CXF file: {e}")))?;
    let major = header
        .get("version")
        .and_then(|v| v.get("major"))
        .and_then(Value::as_u64);
    let accounts = header.get("accounts").and_then(Value::as_array);
    let (Some(1), Some(accounts)) = (major, accounts) else {
        return Err(ImportError::Format(
            "not a CXF 1.x file (credential exchange format)".into(),
        ));
    };
    let mut out = Cxf::default();
    for item in accounts
        .iter()
        .filter_map(|a| a.get("items"))
        .filter_map(Value::as_array)
        .flatten()
    {
        item_of(item, &mut out);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"{"version":{"major":1,"minor":0},"exporterRpId":"other.example","exporterDisplayName":"Other",
      "timestamp":1755129600,"accounts":[{"id":"QQ","username":"","email":"","collections":[],"items":[
        {"id":"AQ","title":"GitHub","favorite":true,"tags":["dev"],"scope":{"urls":["https://github.com"],"androidApps":[]},
         "credentials":[
           {"type":"basic-auth","username":{"fieldType":"string","value":"octo"},"password":{"fieldType":"concealed-string","value":"pw"}},
           {"type":"totp","secret":"JBSWY3DPEHPK3PXP","period":30,"digits":6,"algorithm":"sha1"},
           {"type":"passkey","credentialId":"Y3JlZA","rpId":"github.com","username":"octo","userDisplayName":"Octo","userHandle":"dQ","key":"AQID"},
           {"type":"custom-fields","label":"Recovery","fields":[{"fieldType":"concealed-string","value":"R1","label":"code"},{"fieldType":"date","value":"2026-01-02","label":"made"}]},
           {"type":"note","content":{"fieldType":"string","value":"hello"}},
           {"type":"file","id":"Rg","name":"a.pdf","decryptedSize":3,"integrityHash":"AA"}]},
        {"id":"Ag","title":"Visa","credentials":[{"type":"credit-card","number":{"fieldType":"concealed-string","value":"4111"},
           "expiryDate":{"fieldType":"year-month","value":"2029-04"},"fullName":{"fieldType":"string","value":"Ana"}}]},
        {"id":"Aw","title":"Home Wi-Fi","credentials":[{"type":"wifi","ssid":{"fieldType":"string","value":"casa"},
           "passphrase":{"fieldType":"concealed-string","value":"secret"}}]},
        {"id":"BA","title":"Server","credentials":[{"type":"ssh-key","keyType":"ssh-ed25519","privateKey":"AA",
           "keyGenerationSource":{"fieldType":"string","value":"laptop"}}]},
        {"id":"BQ","title":"Odd","credentials":[{"type":"totp","secret":"JBSWY3DPEHPK3PXP","digits":8,"period":60,"algorithm":"sha256"},
           {"type":"something-new","code":{"fieldType":"string","value":"x1"}}]},
        {"id":"Bg","title":"Empty","credentials":[]}
      ]}]}"#;

    #[test]
    fn reads_logins_passkeys_codes_cards_and_other_kinds() {
        let out = parse(FILE).unwrap();
        assert_eq!((out.items.len(), out.skipped), (5, 1));
        let login = &out.items[0];
        assert_eq!(
            (
                login.kind,
                login.username.as_deref(),
                login.password.as_deref()
            ),
            (ItemKind::Login, Some("octo"), Some("pw"))
        );
        assert_eq!(
            (login.urls.clone(), login.tags.clone(), login.favorite),
            (
                vec!["https://github.com".to_owned()],
                vec!["dev".to_owned()],
                true
            )
        );
        assert_eq!(
            (login.totp.as_deref(), login.notes.as_deref()),
            (Some("JBSWY3DPEHPK3PXP"), Some("hello"))
        );
        assert_eq!(
            login.fields[0],
            ImportedField {
                section: "Recovery".into(),
                label: "code".into(),
                value: "R1".into(),
                concealed: true,
                input: "text"
            }
        );
        assert_eq!(login.fields[1].input, "date");
        let passkey = &out.passkeys[0];
        assert_eq!(
            (
                passkey.item,
                passkey.rp_id.as_str(),
                passkey.user_name.as_str(),
                passkey.key.as_slice()
            ),
            (0, "github.com", "octo", [1u8, 2, 3].as_slice())
        );
        assert!(
            !format!("{passkey:?}").contains("[1, 2, 3]"),
            "the key is never printed"
        );

        let card = &out.items[1];
        assert_eq!(card.kind, ItemKind::CreditCard);
        let value = |item: &ImportedItem, label: &str| {
            item.fields
                .iter()
                .find(|f| f.label == label)
                .unwrap()
                .clone()
        };
        assert!(value(card, "number").concealed);
        assert_eq!(
            (
                value(card, "expiry date").value,
                value(card, "expiry date").input
            ),
            ("2029-04".into(), "month_year")
        );
        assert_eq!(value(card, "cardholder name").value, "Ana");

        let wifi = &out.items[2];
        assert_eq!(
            (
                wifi.kind,
                value(wifi, "ssid").value.as_str(),
                value(wifi, "passphrase").concealed
            ),
            (ItemKind::WirelessRouter, "casa", true)
        );
        assert_eq!(
            (
                out.items[3].kind,
                value(&out.items[3], "key generation source").value.as_str()
            ),
            (ItemKind::SshKey, "laptop")
        );
        let odd = &out.items[4];
        assert_eq!(
            odd.totp.as_deref(),
            Some(
                "otpauth://totp/Imported?secret=JBSWY3DPEHPK3PXP&algorithm=SHA256&digits=8&period=60"
            )
        );
        assert_eq!(value(odd, "code").section, "something new");
        assert_eq!(
            out.warnings,
            [
                "GitHub: attached files come only through a direct transfer, not in a file",
                "Server: the SSH private key was not imported (add it from the key file)"
            ]
        );
    }

    #[test]
    fn refuses_other_files_and_odd_passkeys() {
        assert!(parse("{").is_err());
        assert!(parse(r#"{"version":{"major":2,"minor":0},"accounts":[]}"#).is_err());
        assert!(parse(r#"{"version":{"major":1},"items":[]}"#).is_err());
        let two = r#"{"version":{"major":1,"minor":0},"accounts":[{"items":[{"title":"","credentials":[
            {"type":"passkey","credentialId":"QQ","rpId":"a.com","userHandle":"QQ","key":"AQ"},
            {"type":"passkey","credentialId":"Qg","rpId":"a.com","userHandle":"QQ","key":"AQ"},
            {"type":"passkey","rpId":"a.com"}]}]}]}"#;
        let out = parse(two).unwrap();
        assert_eq!(out.passkeys.len(), 1);
        assert_eq!(
            out.warnings,
            [
                "(untitled): only one passkey per login is kept",
                "(untitled): a passkey was incomplete"
            ]
        );
        assert_eq!(words("documentNumber"), "document number");
    }
}
