//! Export in the FIDO Alliance Credential Exchange Format (CXF 1.0): one
//! JSON file other password managers import, with logins, passkeys,
//! one-time passwords, cards, notes and every other field. The file is
//! **plaintext**, like the CSV export. Attached files are not included (CXF
//! carries them only through the live exchange protocol).

use std::io::Write;
use std::path::Path;

use password365_crypto::passkey::PasskeyKey;
use password365_crypto::secret::SecretString;
use password365_crypto::totp::Totp;
use password365_model::item_template::FieldInput;
use password365_model::{Item, ItemKind, VaultId};
use password365_vault::Session;
use password365_webauthn::{b64url, from_b64url};
use serde_json::{Value, json};

use crate::field_layout::{FieldMeta, describe};
use crate::service::parse_id;
use crate::{AppError, AppService, Result};

/// Who made the file.
const EXPORTER_RP_ID: &str = "password365.app";
const EXPORTER_NAME: &str = "Password365";

fn field(field_type: &str, value: &str) -> Value {
    json!({ "fieldType": field_type, "value": value })
}

fn field_type(meta: &FieldMeta, concealed: bool) -> &'static str {
    if concealed {
        return "concealed-string";
    }
    match meta.input {
        FieldInput::Date => "date",
        FieldInput::MonthYear => "year-month",
        FieldInput::Email => "email",
        _ => "string",
    }
}

fn seconds(t: password365_model::Timestamp) -> i64 {
    t.as_millis() / 1000
}

/// A card's fields as CXF names them (template labels → members).
fn card_member(label: &str) -> Option<&'static str> {
    Some(match label.to_ascii_lowercase().as_str() {
        "cardholder name" => "fullName",
        "number" => "number",
        "type" => "cardType",
        "verification number" => "verificationNumber",
        "pin" => "pin",
        "expiry date" => "expiryDate",
        "valid from" => "validFrom",
        _ => return None,
    })
}

/// Password, passkey and one-time password of an item.
fn sign_in_credentials(item: &Item, username: Option<&str>) -> Vec<Value> {
    let mut credentials: Vec<Value> = Vec::new();
    let password = item
        .details
        .password
        .as_ref()
        .map(SecretString::expose)
        .filter(|p| !p.is_empty());
    if matches!(item.kind, ItemKind::Login | ItemKind::Password)
        && (username.is_some() || password.is_some())
    {
        let mut basic = json!({ "type": "basic-auth" });
        if let Some(u) = username {
            basic["username"] = field("string", u);
        }
        if let Some(p) = password {
            basic["password"] = field("concealed-string", p);
        }
        credentials.push(basic);
    }
    if let Some(passkey) = crate::passkeys::passkey_of(&item.details.custom_fields)
        && let Some(key) = from_b64url(&passkey.key)
            .ok()
            .and_then(|k| PasskeyKey::from_secret_bytes(&k).ok())
    {
        credentials.push(json!({
            "type": "passkey",
            "credentialId": passkey.credential_id,
            "rpId": passkey.rp_id,
            "username": passkey.user_name,
            "userDisplayName": passkey.user_name,
            "userHandle": passkey.user_handle,
            "key": b64url(&key.pkcs8_der()),
        }));
    }
    if let Some(totp) = item
        .details
        .totp
        .as_ref()
        .and_then(|t| Totp::parse(t.expose()).ok())
    {
        let mut code = json!({
            "type": "totp",
            "secret": totp.secret_base32(),
            "period": totp.period(),
            "digits": totp.digits(),
            "algorithm": totp.algorithm_name(),
        });
        if let Some(u) = username {
            code["username"] = json!(u);
        }
        credentials.push(code);
    }
    credentials
}

/// A card's own members, then every other field grouped by section.
fn field_credentials(item: &Item) -> Vec<Value> {
    let mut credentials: Vec<Value> = Vec::new();
    let mut card = json!({ "type": "credit-card" });
    let mut sections: Vec<(String, Vec<Value>)> = Vec::new();
    for (f, meta) in describe(item.kind, &item.details.custom_fields) {
        let value = f.value.expose();
        if value.is_empty() || meta.input == FieldInput::SignInWith {
            continue;
        }
        if item.kind == ItemKind::CreditCard
            && let Some(member) = card_member(&f.label)
            && card.get(member).is_none()
        {
            card[member] = field(field_type(&meta, f.concealed), value);
            continue;
        }
        let entry = json!({ "fieldType": field_type(&meta, f.concealed), "value": value, "label": f.label });
        match sections.iter_mut().find(|(s, _)| *s == meta.section) {
            Some((_, fields)) => fields.push(entry),
            None => sections.push((meta.section.clone(), vec![entry])),
        }
    }
    if card.as_object().is_some_and(|o| o.len() > 1) {
        credentials.push(card);
    }
    for (section, fields) in sections {
        let mut custom = json!({ "type": "custom-fields", "fields": fields });
        if !section.is_empty() {
            custom["label"] = json!(section);
        }
        credentials.push(custom);
    }
    credentials
}

/// One item as a CXF `Item`.
#[must_use]
pub fn item_json(item: &Item) -> Value {
    let username = item.overview.username.as_deref().filter(|u| !u.is_empty());
    let mut credentials = sign_in_credentials(item, username);
    credentials.extend(field_credentials(item));
    if let Some(notes) = item
        .details
        .notes
        .as_ref()
        .map(SecretString::expose)
        .filter(|n| !n.is_empty())
    {
        credentials.push(json!({ "type": "note", "content": field("string", notes) }));
    }

    let mut out = json!({
        "id": b64url(item.id.as_bytes()),
        "creationAt": seconds(item.created_at),
        "modifiedAt": seconds(item.updated_at),
        "title": item.overview.title,
        "favorite": item.overview.favorite,
        "credentials": credentials,
        "tags": item.overview.tags,
    });
    if let Some(u) = username {
        out["subtitle"] = json!(u);
    }
    if !item.overview.urls.is_empty() {
        out["scope"] = json!({ "urls": item.overview.urls, "androidApps": [] });
    }
    out
}

/// The whole export: a CXF `Header` with one account.
pub fn export_json(
    session: &Session,
    vault: Option<VaultId>,
    now_seconds: i64,
) -> Result<(Value, usize)> {
    let items: Vec<Value> = session
        .items()
        .into_iter()
        .filter(|s| vault.is_none_or(|v| s.vault_id == v))
        .map(|s| session.item(s.id).map(|item| item_json(&item)))
        .collect::<std::result::Result<_, _>>()?;
    let count = items.len();
    let header = json!({
        "version": { "major": 1, "minor": 0 },
        "exporterRpId": EXPORTER_RP_ID,
        "exporterDisplayName": EXPORTER_NAME,
        "timestamp": now_seconds,
        "accounts": [{
            "id": b64url(&password365_crypto::random::bytes::<16>()?),
            "username": "",
            "email": "",
            "collections": [],
            "items": items,
        }],
    });
    Ok((header, count))
}

impl AppService {
    /// Exports items (optionally of one vault) to a new owner-only CXF file
    /// (PLAINTEXT), once the master password is typed again. Returns how many
    /// items were written.
    pub fn export_cxf(
        &mut self,
        path: &Path,
        vault_id: Option<&str>,
        password: password365_crypto::secret::SecretString,
    ) -> Result<usize> {
        self.confirm_master_password(password)?;
        let vault = vault_id.map(parse_id::<VaultId>).transpose()?;
        let session = self.session()?;
        if let Some(v) = vault {
            session.vault(v)?;
        }
        let now = password365_model::Timestamp::now().as_millis() / 1000;
        let (json, count) = export_json(session, vault, now)?;
        let text = zeroize::Zeroizing::new(
            serde_json::to_vec_pretty(&json).map_err(|e| AppError::Internal(e.to_string()))?,
        );
        password365_vault::private_file::create_private_file(path)
            .and_then(|mut f| f.write_all(&text))
            .map_err(|e| AppError::Io(format!("cannot create {}: {e}", path.display())))?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item_input::{FieldValue, ItemInput};
    use crate::service::tests::{Fixture, PW, with_account};

    fn custom(label: &str, value: &str) -> FieldValue {
        FieldValue {
            label: label.into(),
            value: value.into(),
            ..FieldValue::default()
        }
    }

    /// Exports everything and returns the file's items by title.
    fn export(fx: &mut Fixture) -> (tempfile::TempDir, std::path::PathBuf, Vec<Value>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.cxf.json");
        fx.app.export_cxf(&path, None, PW.into()).unwrap();
        let json: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["version"], json!({"major": 1, "minor": 0}));
        assert_eq!(json["exporterRpId"], "password365.app");
        let items = json["accounts"][0]["items"].as_array().unwrap().clone();
        (dir, path, items)
    }

    #[test]
    fn exports_a_passkey_as_pkcs8_and_keeps_internal_fields_inside() {
        let mut fx = with_account();
        let request = password365_webauthn::authenticator::CreateRequest {
            rp_id: "example.com".into(),
            user_id: b64url(b"u"),
            user_name: "ana".into(),
            challenge: b64url(b"c"),
            algorithms: vec![-7],
            user_verification: password365_webauthn::authenticator::UserVerification::Preferred,
        };
        fx.app
            .passkey_register("https://example.com", "Example", &request, &[], None, None)
            .unwrap();
        let (_dir, path, items) = export(&mut fx);
        let credentials = items[0]["credentials"].as_array().unwrap();
        let passkey = credentials.iter().find(|c| c["type"] == "passkey").unwrap();
        assert_eq!(
            (passkey["rpId"].as_str(), passkey["username"].as_str()),
            (Some("example.com"), Some("ana"))
        );
        assert_eq!(
            from_b64url(passkey["key"].as_str().unwrap()).unwrap().len(),
            138,
            "PKCS#8"
        );
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("password365 passkey"),
            "internal fields stay inside"
        );
        assert!(
            fx.app.export_cxf(&path, None, PW.into()).is_err(),
            "never overwrites a file"
        );
    }

    #[test]
    fn exports_logins_codes_cards_and_fields() {
        let mut fx = with_account();
        fx.app
            .create_item(&ItemInput {
                title: "GitHub".into(),
                username: Some("octo".into()),
                urls: vec!["https://github.com".into()],
                password: Some("pw".into()),
                totp: Some("JBSWY3DPEHPK3PXP".into()),
                notes: Some("note".into()),
                tags: vec!["dev".into()],
                favorite: true,
                fields: Some(vec![custom("recovery code", "R1")]),
                ..ItemInput::default()
            })
            .unwrap();
        fx.app
            .create_item(&ItemInput {
                kind: Some("card".into()),
                title: "Visa".into(),
                fields: Some(vec![
                    custom("number", "4111"),
                    custom("expiry date", "2029-04"),
                    custom("bank", "Itaú"),
                ]),
                ..ItemInput::default()
            })
            .unwrap();
        let (dir, _path, items) = export(&mut fx);
        assert_eq!(items.len(), 2);
        let by_title = |t: &str| items.iter().find(|i| i["title"] == t).unwrap().clone();

        let github = by_title("GitHub");
        assert_eq!(
            (github["subtitle"].as_str(), github["favorite"].as_bool()),
            (Some("octo"), Some(true))
        );
        assert_eq!(github["scope"]["urls"], json!(["https://github.com"]));
        assert_eq!(github["tags"], json!(["dev"]));
        let creds = github["credentials"].as_array().unwrap();
        let kinds: Vec<&str> = creds.iter().map(|c| c["type"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["basic-auth", "totp", "custom-fields", "note"]);
        assert_eq!(
            creds[0]["password"],
            json!({"fieldType": "concealed-string", "value": "pw"})
        );
        assert_eq!(
            (
                creds[1]["secret"].as_str(),
                creds[1]["digits"].as_u64(),
                creds[1]["algorithm"].as_str()
            ),
            (Some("JBSWY3DPEHPK3PXP"), Some(6), Some("sha1"))
        );
        assert_eq!(creds[2]["fields"][0]["label"], "recovery code");

        let visa = by_title("Visa")["credentials"].clone();
        assert_eq!(visa[0]["type"], "credit-card");
        assert_eq!(visa[0]["number"]["value"], "4111");
        assert_eq!(
            visa[0]["expiryDate"],
            json!({"fieldType": "year-month", "value": "2029-04"})
        );
        assert_eq!(
            visa[1]["fields"][0]["value"], "Itaú",
            "fields CXF has no member for stay as custom fields"
        );

        assert!(
            fx.app
                .export_cxf(&dir.path().join("x"), Some("not-a-vault"), PW.into())
                .is_err()
        );
    }

    #[test]
    fn an_export_imports_into_another_account_and_its_passkey_still_signs_in() {
        let mut from = with_account();
        from.app
            .create_item(&ItemInput {
                title: "Mail".into(),
                username: Some("ana".into()),
                password: Some("pw".into()),
                totp: Some("JBSWY3DPEHPK3PXP".into()),
                ..ItemInput::default()
            })
            .unwrap();
        let request = password365_webauthn::authenticator::CreateRequest {
            rp_id: "example.com".into(),
            user_id: b64url(b"u"),
            user_name: "ana".into(),
            challenge: b64url(b"c"),
            algorithms: vec![-7],
            user_verification: password365_webauthn::authenticator::UserVerification::Preferred,
        };
        let (_, registration) = from
            .app
            .passkey_register("https://example.com", "Example", &request, &[], None, None)
            .unwrap();
        let (dir, path, _) = export(&mut from);

        let mut to = with_account();
        let summary = to.app.import_cxf(&path, None).unwrap();
        assert_eq!(
            (summary.imported, summary.skipped.len()),
            (2, 0),
            "{summary:?}"
        );
        let mail = to.app.items("Mail", None).unwrap().remove(0);
        let mail = to.app.item(&mail.id).unwrap();
        assert!(mail.has_password && mail.has_totp);
        let example = to.app.items("Example", None).unwrap().remove(0).id;
        assert_eq!(
            to.app.item(&example).unwrap().passkey.map(|p| p.user_name),
            Some("ana".to_owned())
        );

        // The imported key is the registered one: its signature verifies.
        let answer = to
            .app
            .passkey_assert(
                "https://example.com",
                &example,
                &b64url(b"c2"),
                password365_webauthn::authenticator::UserVerification::Preferred,
                None,
            )
            .unwrap();
        assert_eq!(answer.credential_id, registration.credential_id);
        let spki = from_b64url(&registration.public_key).unwrap();
        let (x, y) = (
            spki[27..59].try_into().unwrap(),
            spki[59..91].try_into().unwrap(),
        );
        let mut signed = from_b64url(&answer.authenticator_data).unwrap();
        signed.extend_from_slice(&<sha2::Sha256 as sha2::Digest>::digest(
            from_b64url(&answer.client_data_json).unwrap(),
        ));
        assert!(password365_crypto::passkey::verify(
            &x,
            &y,
            &signed,
            &from_b64url(&answer.signature).unwrap()
        ));

        let again = to.app.import_cxf(&path, None).unwrap();
        assert_eq!((again.imported, again.identical), (0, 2), "nothing twice");
        std::fs::write(dir.path().join("bad.json"), b"{}").unwrap();
        assert!(
            to.app
                .import_cxf(&dir.path().join("bad.json"), None)
                .is_err()
        );
    }
}
