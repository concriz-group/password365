//! What the iOS `AutoFill` extension needs: the list of logins and passkeys
//! the system suggests above the keyboard (no secrets), and the values of
//! the one the user picked.

use password365_app::dto::SecretField;

use crate::{ItemRow, Password365, Result};

/// A login the system can suggest (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LoginIdentity {
    /// Item id.
    pub item_id: String,
    /// Username.
    pub username: String,
    /// Websites.
    pub urls: Vec<String>,
}

/// A passkey the system can suggest (no key).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PasskeyIdentity {
    /// Item id.
    pub item_id: String,
    /// Relying-party id.
    pub rp_id: String,
    /// Account name.
    pub user_name: String,
    /// The site's id for the account.
    pub user_handle: Vec<u8>,
    /// Credential id.
    pub credential_id: Vec<u8>,
}

/// Username and password to fill.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FillValues {
    /// Username ("" for none).
    pub username: String,
    /// Password ("" for none).
    pub password: String,
}

/// A passkey sign-in for the system.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PasskeySignIn {
    /// Credential id.
    pub credential_id: Vec<u8>,
    /// `authenticatorData`.
    pub authenticator_data: Vec<u8>,
    /// Signature.
    pub signature: Vec<u8>,
    /// The site's id for the account.
    pub user_handle: Vec<u8>,
}

/// A new passkey for the system.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PasskeyCreated {
    /// Credential id.
    pub credential_id: Vec<u8>,
    /// `attestationObject`.
    pub attestation_object: Vec<u8>,
}

/// The page or app the system names (`example.com` or a full URL).
fn page_url(service: &str) -> String {
    let service = service.trim();
    if service.contains("://") {
        service.to_owned()
    } else {
        format!("https://{service}")
    }
}

fn bytes(text: &str) -> Vec<u8> {
    password365_webauthn::from_b64url(text).unwrap_or_default()
}

#[uniffi::export]
impl Password365 {
    /// Every active login with a username or website (for the system's suggestions).
    pub fn login_identities(&self) -> Result<Vec<LoginIdentity>> {
        Ok(self
            .app()
            .items("", None)?
            .into_iter()
            .filter(|s| s.kind == "login" && (s.username.is_some() || !s.urls.is_empty()))
            .map(|s| LoginIdentity {
                item_id: s.id,
                username: s.username.unwrap_or_default(),
                urls: s.urls,
            })
            .collect())
    }

    /// Every passkey (for the system's suggestions).
    pub fn passkey_identities(&self) -> Result<Vec<PasskeyIdentity>> {
        Ok(self
            .app()
            .passkey_identities()?
            .into_iter()
            .map(|(item_id, p)| PasskeyIdentity {
                item_id,
                user_handle: bytes(&p.user_handle),
                credential_id: bytes(&p.credential_id),
                rp_id: p.rp_id,
                user_name: p.user_name,
            })
            .collect())
    }

    /// Logins for a website or app's domain, best match first.
    pub fn logins_for_service(&self, service: String) -> Result<Vec<ItemRow>> {
        Ok(self
            .app()
            .logins_for_url(&page_url(&service))?
            .into_iter()
            .map(|c| ItemRow {
                id: c.id,
                title: c.title,
                subtitle: c.username.unwrap_or_default(),
                kind: "login".into(),
                favorite: false,
            })
            .collect())
    }

    /// Username and password of `item_id` for `service` (the site is checked
    /// against the item's websites, like the browser extension). An empty
    /// `service` fills an item the user picked from the whole list.
    pub fn fill(&self, item_id: String, service: String) -> Result<FillValues> {
        let mut app = self.app();
        if service.trim().is_empty() {
            let view = app.item(&item_id)?;
            let password = app
                .reveal(&item_id, SecretField::Password)?
                .unwrap_or_default();
            return Ok(FillValues {
                username: view.summary.username.unwrap_or_default(),
                password,
            });
        }
        let creds = app.fill_credentials(&item_id, &page_url(&service))?;
        Ok(FillValues {
            username: creds.username.unwrap_or_default(),
            password: creds.password.unwrap_or_default(),
        })
    }

    /// The current one-time code of `item_id`.
    pub fn one_time_code(&self, item_id: String) -> Result<Option<String>> {
        Ok(self.app().totp_code(&item_id, None)?.map(|c| c.code))
    }

    /// Signs in with a passkey for the system (which built the client data).
    pub fn passkey_sign_in(
        &self,
        rp_id: String,
        credential_id: Vec<u8>,
        client_data_hash: Vec<u8>,
    ) -> Result<PasskeySignIn> {
        let a = self
            .app()
            .passkey_assert_platform(&rp_id, &credential_id, &client_data_hash)?;
        Ok(PasskeySignIn {
            credential_id: a.credential_id,
            authenticator_data: a.authenticator_data,
            signature: a.signature,
            user_handle: a.user_handle,
        })
    }

    /// Creates a passkey for the system.
    pub fn passkey_create(
        &self,
        rp_id: String,
        user_handle: Vec<u8>,
        user_name: String,
        algorithms: Vec<i64>,
    ) -> Result<PasskeyCreated> {
        let r =
            self.app()
                .passkey_register_platform(&rp_id, &user_handle, &user_name, &algorithms)?;
        Ok(PasskeyCreated {
            credential_id: r.credential_id,
            attestation_object: r.attestation_object,
        })
    }
}
