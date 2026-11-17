//! Passkeys kept in login items, like 1Password: a login holds at most one
//! passkey, in an internal field (so it syncs field by field and needs no new
//! item format). Its private key never leaves this process; the browser gets
//! only the signed answers.

use password365_crypto::secret::SecretString;
use password365_model::{CustomField, Item, ItemId, ItemKind, ItemOverview, Timestamp};
use password365_webauthn::authenticator::{self, CreateRequest, UserVerification};
use password365_webauthn::{Assertion, Passkey, Registration, WebAuthnError};
use serde::Serialize;

use crate::service::parse_id;
use crate::{AppError, AppService, Result};

/// Label of the internal field. Never shown, never accepted from a form.
pub const PASSKEY_LABEL: &str = "password365 passkey";

/// A passkey as the item view shows it (no key).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PasskeyDto {
    /// Site it signs in to.
    pub rp_id: String,
    /// Account name at that site.
    pub user_name: String,
}

/// A login the page can sign in with (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PasskeyOption {
    /// Item id.
    pub item_id: String,
    /// Item title.
    pub title: String,
    /// Account name.
    pub user_name: String,
}

impl From<WebAuthnError> for AppError {
    fn from(e: WebAuthnError) -> Self {
        Self::Validation(e.to_string())
    }
}

/// The passkey stored in `fields`, if any.
#[must_use]
pub fn passkey_of(fields: &[CustomField]) -> Option<Passkey> {
    fields
        .iter()
        .find(|f| f.label.eq_ignore_ascii_case(PASSKEY_LABEL))
        .and_then(|f| serde_json::from_str(f.value.expose()).ok())
}

/// The view of the passkey in `fields`.
#[must_use]
pub fn dto_of(fields: &[CustomField]) -> Option<PasskeyDto> {
    passkey_of(fields).map(|p| PasskeyDto {
        rp_id: p.rp_id,
        user_name: p.user_name,
    })
}

fn set_passkey(fields: &mut Vec<CustomField>, passkey: Option<&Passkey>) -> Result<()> {
    fields.retain(|f| !f.label.eq_ignore_ascii_case(PASSKEY_LABEL));
    if let Some(passkey) = passkey {
        let json = serde_json::to_string(passkey).map_err(|e| AppError::Internal(e.to_string()))?;
        fields.push(CustomField {
            label: PASSKEY_LABEL.to_owned(),
            value: json.into(),
            concealed: true,
        });
    }
    Ok(())
}

/// Puts back the passkey of `before` after a form replaced the fields (the
/// form never shows or sends it).
pub fn keep_passkey(before: &[CustomField], after: &mut Vec<CustomField>) -> Result<()> {
    if passkey_of(after).is_none() {
        set_passkey(after, passkey_of(before).as_ref())?;
    }
    Ok(())
}

impl AppService {
    /// Every active login with a passkey, with that passkey.
    fn passkeys(&mut self) -> Result<Vec<(Item, Passkey)>> {
        let session = self.session()?;
        let ids: Vec<ItemId> = session
            .items()
            .into_iter()
            .filter(|s| s.kind == ItemKind::Login && crate::service::active(s))
            .map(|s| s.id)
            .collect();
        let mut out = Vec::new();
        for id in ids {
            let item = session.item(id)?;
            if let Some(passkey) = passkey_of(&item.details.custom_fields) {
                out.push((item, passkey));
            }
        }
        Ok(out)
    }

    /// Creates a passkey for the page at `page_url` (the tab's real address).
    /// `exclude` lists the site's credential ids for this account: if one is
    /// here already, nothing is created. It goes into a login of that site
    /// with the same account name and no passkey yet, or a new login.
    pub fn passkey_register(
        &mut self,
        page_url: &str,
        title: &str,
        request: &CreateRequest,
        exclude: &[String],
        vault_id: Option<&str>,
        password: Option<SecretString>,
    ) -> Result<(String, Registration)> {
        let verified = self.passkey_verification(request.user_verification, password)?;
        let existing = self.passkeys()?;
        let rp_id = request.rp_id.trim().to_ascii_lowercase();
        if existing
            .iter()
            .any(|(_, p)| p.rp_id == rp_id && exclude.contains(&p.credential_id))
        {
            return Err(AppError::Validation(
                "This account already has a passkey in Password365.".into(),
            ));
        }
        let (passkey, registration) = authenticator::register(page_url, request, verified)?;
        let id = self.keep_new_passkey(page_url, title, &passkey, &existing, vault_id)?;
        Ok((id, registration))
    }

    /// Puts a new passkey in the login of that site with the same account
    /// name and no passkey yet, or in a new login; returns the item id.
    fn keep_new_passkey(
        &mut self,
        page_url: &str,
        title: &str,
        passkey: &Passkey,
        existing: &[(Item, Passkey)],
        vault_id: Option<&str>,
    ) -> Result<String> {
        let origin = password365_match::Site::parse(page_url)
            .map(|s| s.origin())
            .unwrap_or_default();
        let same_account = self
            .logins_for_url(page_url)?
            .into_iter()
            .filter(|c| c.username.as_deref() == Some(passkey.user_name.as_str()))
            .map(|c| c.id)
            .find(|id| !existing.iter().any(|(item, _)| item.id.to_string() == *id));
        let mut item = if let Some(id) = same_account {
            self.load_item(&id)?
        } else {
            let vault = self.vault_or_default(vault_id)?;
            let name = if title.trim().is_empty() {
                passkey.rp_id.as_str()
            } else {
                title.trim()
            };
            let mut overview = ItemOverview::new(name)
                .map_err(|e| AppError::Validation(e.to_string()))?
                .with_url(&origin);
            if !passkey.user_name.is_empty() {
                overview = overview.with_username(&passkey.user_name);
            }
            let id = self.session()?.add_item(
                vault,
                ItemKind::Login,
                overview,
                password365_model::ItemDetails::default(),
            )?;
            self.session()?.item(id)?
        };
        set_passkey(&mut item.details.custom_fields, Some(passkey))?;
        item.touch(Timestamp::now());
        self.session()?.save_item(&item)?;
        Ok(item.id.to_string())
    }

    /// Creates a passkey for the phone's `AutoFill` (the system checked the
    /// app or site and builds the client data itself).
    pub fn passkey_register_platform(
        &mut self,
        rp_id: &str,
        user_handle: &[u8],
        user_name: &str,
        algorithms: &[i64],
    ) -> Result<authenticator::PlatformRegistration> {
        let existing = self.passkeys()?;
        let (passkey, registration) = authenticator::register_for_platform(
            rp_id,
            user_handle,
            user_name,
            algorithms,
            self.verified_recently(),
        )?;
        let page = format!("https://{}", passkey.rp_id);
        self.keep_new_passkey(&page, "", &passkey, &existing, None)?;
        Ok(registration)
    }

    /// Signs in with the passkey `credential_id` (raw bytes) of `rp_id` for
    /// the phone's `AutoFill`, over the system's client data hash.
    pub fn passkey_assert_platform(
        &mut self,
        rp_id: &str,
        credential_id: &[u8],
        client_data_hash: &[u8],
    ) -> Result<authenticator::PlatformAssertion> {
        let wanted = password365_webauthn::b64url(credential_id);
        let rp_id = rp_id.trim().to_ascii_lowercase();
        let (_, passkey) = self
            .passkeys()?
            .into_iter()
            .find(|(_, p)| p.rp_id == rp_id && p.credential_id == wanted)
            .ok_or(AppError::NotFound("passkey"))?;
        Ok(authenticator::assert_for_platform(
            &passkey,
            client_data_hash,
            self.verified_recently(),
        )?)
    }

    /// Every passkey (site, account and ids; never the key), for the
    /// phone's list of `AutoFill` suggestions.
    pub fn passkey_identities(&mut self) -> Result<Vec<(String, Passkey)>> {
        Ok(self
            .passkeys()?
            .into_iter()
            .map(|(item, mut p)| {
                p.key = String::new();
                (item.id.to_string(), p)
            })
            .collect())
    }

    /// Logins whose passkey the page may use for `rp_id` (only those in
    /// `allow` when the site names them).
    pub fn passkey_options(
        &mut self,
        page_url: &str,
        rp_id: &str,
        allow: &[String],
    ) -> Result<Vec<PasskeyOption>> {
        password365_webauthn::rp::check(page_url, rp_id)?;
        let rp_id = rp_id.trim().to_ascii_lowercase();
        let mut out: Vec<PasskeyOption> = self
            .passkeys()?
            .into_iter()
            .filter(|(_, p)| {
                p.rp_id == rp_id && (allow.is_empty() || allow.contains(&p.credential_id))
            })
            .map(|(item, p)| PasskeyOption {
                item_id: item.id.to_string(),
                title: item.overview.title,
                user_name: p.user_name,
            })
            .collect();
        out.sort_by(|a, b| {
            a.title
                .cmp(&b.title)
                .then_with(|| a.user_name.cmp(&b.user_name))
        });
        Ok(out)
    }

    /// Signs in to the page with the passkey of item `id`.
    /// `wanted` is the page's `userVerification`; `password`, when given,
    /// is the master password typed to verify the user.
    pub fn passkey_assert(
        &mut self,
        page_url: &str,
        id: &str,
        challenge: &str,
        wanted: UserVerification,
        password: Option<SecretString>,
    ) -> Result<Assertion> {
        let item = self.load_item(id)?;
        let passkey =
            passkey_of(&item.details.custom_fields).ok_or(AppError::NotFound("passkey"))?;
        let verified = self.passkey_verification(wanted, password)?;
        Ok(authenticator::assert(
            page_url, &passkey, challenge, verified,
        )?)
    }

    /// Whether this passkey answer may say the user was verified: a master
    /// password typed now, or an unlock within [`crate::service::VERIFIED_FOR`].
    /// A site that requires verification gets `VerificationNeeded` otherwise.
    fn passkey_verification(
        &mut self,
        wanted: UserVerification,
        password: Option<SecretString>,
    ) -> Result<bool> {
        if let Some(password) = password {
            self.confirm_master_password(password)?;
        }
        let verified = self.verified_recently();
        if wanted == UserVerification::Required && !verified {
            return Err(AppError::VerificationNeeded);
        }
        Ok(verified && wanted != UserVerification::Discouraged)
    }

    /// Gives login `id` an imported passkey (its key as PKCS#8 DER).
    pub(crate) fn import_passkey(
        &mut self,
        id: &str,
        imported: &password365_import::cxf::CxfPasskey,
    ) -> Result<()> {
        use password365_crypto::passkey::PasskeyKey;
        use password365_webauthn::b64url;
        let key = PasskeyKey::from_pkcs8_der(&imported.key)
            .map_err(|_| AppError::Validation("only P-256 (ES256) passkeys can be used".into()))?;
        let passkey = Passkey {
            credential_id: imported.credential_id.clone(),
            rp_id: imported.rp_id.trim().to_ascii_lowercase(),
            user_handle: imported.user_handle.clone(),
            user_name: imported.user_name.clone(),
            key: b64url(&key.secret_bytes()),
        };
        let mut item = self.load_item(id)?;
        if item.kind != ItemKind::Login {
            return Err(AppError::Validation("passkeys belong to logins".into()));
        }
        set_passkey(&mut item.details.custom_fields, Some(&passkey))?;
        item.touch(Timestamp::now());
        Ok(self.session()?.save_item(&item)?)
    }

    /// Removes the passkey from item `id`.
    pub fn remove_passkey(&mut self, id: &str) -> Result<()> {
        let mut item = self.load_item(&parse_id::<ItemId>(id)?.to_string())?;
        set_passkey(&mut item.details.custom_fields, None)?;
        item.touch(Timestamp::now());
        Ok(self.session()?.save_item(&item)?)
    }
}

#[cfg(test)]
mod tests {
    use password365_webauthn::b64url;

    use super::*;
    use crate::item_input::ItemInput;
    use crate::service::tests::{PW, with_account};
    use password365_webauthn::from_b64url;

    fn request(user: &str) -> CreateRequest {
        CreateRequest {
            rp_id: "example.com".into(),
            user_id: b64url(user.as_bytes()),
            user_name: user.into(),
            challenge: b64url(b"c1"),
            algorithms: vec![-7],
            user_verification: UserVerification::Preferred,
        }
    }

    #[test]
    fn a_site_that_requires_verification_gets_it_only_when_it_happened() {
        let mut fx = with_account();
        let (id, _) = fx
            .app
            .passkey_register("https://example.com", "", &request("ana"), &[], None, None)
            .unwrap();
        let uv = |a: &Assertion| from_b64url(&a.authenticator_data).unwrap()[32] & 0x04;
        // Just unlocked: verified.
        let answer = fx
            .app
            .passkey_assert(
                "https://example.com",
                &id,
                "AA",
                UserVerification::Required,
                None,
            )
            .unwrap();
        assert_eq!(uv(&answer), 0x04);
        // Long after the unlock: a required check needs the master password.
        fx.app.verified_at = None;
        assert!(matches!(
            fx.app.passkey_assert(
                "https://example.com",
                &id,
                "AA",
                UserVerification::Required,
                None
            ),
            Err(AppError::VerificationNeeded)
        ));
        let answer = fx
            .app
            .passkey_assert(
                "https://example.com",
                &id,
                "AA",
                UserVerification::Preferred,
                None,
            )
            .unwrap();
        assert_eq!(uv(&answer), 0, "not claimed when it did not happen");
        assert!(matches!(
            fx.app.passkey_assert(
                "https://example.com",
                &id,
                "AA",
                UserVerification::Required,
                Some(SecretString::from("wrong"))
            ),
            Err(AppError::WrongCredentials)
        ));
        let answer = fx
            .app
            .passkey_assert(
                "https://example.com",
                &id,
                "AA",
                UserVerification::Required,
                Some(SecretString::from(PW)),
            )
            .unwrap();
        assert_eq!(uv(&answer), 0x04);
        let answer = fx
            .app
            .passkey_assert(
                "https://example.com",
                &id,
                "AA",
                UserVerification::Discouraged,
                None,
            )
            .unwrap();
        assert_eq!(uv(&answer), 0);
    }

    #[test]
    fn creates_lists_signs_in_and_removes_passkeys() {
        let mut fx = with_account();
        let (id, reg) = fx
            .app
            .passkey_register(
                "https://example.com/signup",
                "Example",
                &request("ana"),
                &[],
                None,
                None,
            )
            .unwrap();
        let item = fx.app.item(&id).unwrap();
        assert_eq!(
            (
                item.summary.title.as_str(),
                item.summary.username.as_deref()
            ),
            ("Example", Some("ana"))
        );
        assert_eq!(item.summary.urls, ["https://example.com"]);
        assert_eq!(
            item.passkey,
            Some(PasskeyDto {
                rp_id: "example.com".into(),
                user_name: "ana".into()
            })
        );
        assert!(item.fields.is_empty(), "the passkey field is internal");

        let options = fx
            .app
            .passkey_options("https://www.example.com", "example.com", &[])
            .unwrap();
        assert_eq!(
            options,
            [PasskeyOption {
                item_id: id.clone(),
                title: "Example".into(),
                user_name: "ana".into()
            }]
        );
        assert!(
            fx.app
                .passkey_options("https://www.example.com", "example.com", &["other".into()])
                .unwrap()
                .is_empty()
        );
        assert!(
            fx.app
                .passkey_options("https://evil.com", "example.com", &[])
                .is_err()
        );

        let answer = fx
            .app
            .passkey_assert(
                "https://example.com",
                &id,
                &b64url(b"c2"),
                UserVerification::Preferred,
                None,
            )
            .unwrap();
        assert_eq!(answer.credential_id, reg.credential_id);

        let again = fx.app.passkey_register(
            "https://example.com",
            "",
            &request("ana"),
            std::slice::from_ref(&reg.credential_id),
            None,
            None,
        );
        assert!(
            matches!(again, Err(AppError::Validation(_))),
            "excluded: already registered"
        );

        fx.app.remove_passkey(&id).unwrap();
        assert_eq!(fx.app.item(&id).unwrap().passkey, None);
        assert!(
            fx.app
                .passkey_assert(
                    "https://example.com",
                    &id,
                    "AA",
                    UserVerification::Preferred,
                    None
                )
                .is_err()
        );
    }

    #[test]
    fn the_phones_autofill_creates_and_uses_passkeys() {
        let mut fx = with_account();
        let reg = fx
            .app
            .passkey_register_platform("Example.com", b"u-1", "ana", &[-7])
            .unwrap();
        let ids = fx.app.passkey_identities().unwrap();
        assert_eq!(ids.len(), 1);
        let (item_id, identity) = &ids[0];
        assert_eq!(
            (
                identity.rp_id.as_str(),
                identity.user_name.as_str(),
                identity.key.as_str()
            ),
            ("example.com", "ana", ""),
            "no key leaves"
        );
        let item = fx.app.item(item_id).unwrap();
        assert_eq!(
            (item.summary.urls.clone(), item.summary.username.as_deref()),
            (vec!["https://example.com".to_owned()], Some("ana"))
        );
        let answer = fx
            .app
            .passkey_assert_platform("example.com", &reg.credential_id, &[9; 32])
            .unwrap();
        assert_eq!(answer.user_handle, b"u-1");
        assert_eq!(
            fx.app
                .passkey_assert_platform("other.com", &reg.credential_id, &[9; 32])
                .unwrap_err(),
            AppError::NotFound("passkey")
        );
        assert!(
            fx.app
                .passkey_assert_platform("example.com", b"nope", &[9; 32])
                .is_err()
        );
    }

    #[test]
    fn goes_into_the_existing_login_of_that_account_and_survives_edits() {
        let mut fx = with_account();
        let login = fx
            .app
            .create_item(&ItemInput {
                title: "Example".into(),
                username: Some("bob".into()),
                urls: vec!["https://example.com".into()],
                password: Some("pw".into()),
                ..ItemInput::default()
            })
            .unwrap();
        let (id, _) = fx
            .app
            .passkey_register("https://example.com", "", &request("bob"), &[], None, None)
            .unwrap();
        assert_eq!(id, login.summary.id);
        let (other, _) = fx
            .app
            .passkey_register("https://example.com", "", &request("bob"), &[], None, None)
            .unwrap();
        assert_ne!(
            other, id,
            "a login already holding a passkey gets no second one"
        );

        let shown = fx.app.item(&id).unwrap();
        let edit = ItemInput {
            title: "Example (renamed)".into(),
            username: Some("bob".into()),
            fields: Some(vec![]),
            ..ItemInput::default()
        };
        fx.app.update_item(&id, shown.version, &edit).unwrap();
        assert!(
            fx.app.item(&id).unwrap().passkey.is_some(),
            "editing the login keeps its passkey"
        );
        let csv = {
            let mut out = Vec::new();
            crate::transfer::export_csv(fx.app.session().unwrap(), &mut out, None).unwrap();
            String::from_utf8(out).unwrap()
        };
        assert!(!csv.contains("passkey"), "never exported");
    }
}
