//! Password365 for the mobile apps: the same Rust core as the desktop, behind
//! a small interface generated for Swift (and later Kotlin) by UniFFI.
//!
//! Only what a screen shows crosses: listings and item views carry no secret
//! values; passwords, notes and concealed fields cross one at a time when the
//! user reveals or copies them. The Secret Key is kept by the app (iOS
//! Keychain) through [`KeyKeeper`].

// UniFFI hands every argument over owned; borrowing is not an option here.
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use password365_app::dto::SecretField;
use password365_app::secret_key_store::SecretKeyStore;
use password365_app::{AppConfig, AppError, AppService};
use password365_crypto::kdf::KdfParams;
use password365_crypto::secret::SecretString;
use password365_crypto::secret_key::AccountSecretKey;

uniffi::setup_scaffolding!();

mod autofill;
mod editing;
mod emergency_kit;
mod hosting;
mod importing;
pub use autofill::{FillValues, LoginIdentity, PasskeyCreated, PasskeyIdentity, PasskeySignIn};
mod pairing;
mod phone_extras;
mod security;
mod sync_files;
mod watchtower;
pub use editing::{
    Category, DuplicateGroupView, DuplicateMemberView, FieldDraft, Generated, ItemDraft, Recipe,
    TemplateField,
};
pub use importing::ImportResult;
pub use pairing::{Device, SyncSummary};
pub use phone_extras::DeletedItemView;
pub use security::MobileSettings;

/// Where the app keeps the Secret Key (the iOS Keychain), implemented in Swift.
#[uniffi::export(with_foreign)]
pub trait KeyKeeper: Send + Sync {
    /// The stored key text, if any.
    fn load(&self) -> Option<String>;
    /// Stores the key text; `false` if it could not.
    fn save(&self, key: String) -> bool;
    /// Forgets the key.
    fn remove(&self);
}

/// An error a screen can show (`code` is stable, `reason` is English). Not `message`:
/// in Kotlin that would clash with every exception's own `message`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum Failure {
    /// Something went wrong.
    #[error("{reason}")]
    Failed {
        /// Stable identifier (`locked`, `wrong_credentials`, `validation`, …).
        code: String,
        /// What went wrong, in English.
        reason: String,
    },
}

impl From<AppError> for Failure {
    fn from(e: AppError) -> Self {
        Self::Failed {
            code: e.code().to_owned(),
            reason: e.to_string(),
        }
    }
}

type Result<T> = std::result::Result<T, Failure>;

/// Whether an account exists and is unlocked.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Status {
    /// An account exists on this device.
    pub initialized: bool,
    /// The vault is unlocked.
    pub unlocked: bool,
}

/// One row of the item list (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ItemRow {
    /// Id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Username, or "".
    pub subtitle: String,
    /// Category (`login`, `card`, …).
    pub kind: String,
    /// Starred.
    pub favorite: bool,
    /// Tags (for the list's tag filter).
    pub tags: Vec<String>,
}

impl From<password365_app::dto::ItemSummaryDto> for ItemRow {
    fn from(s: password365_app::dto::ItemSummaryDto) -> Self {
        Self {
            id: s.id,
            title: s.title,
            subtitle: s.username.unwrap_or_default(),
            kind: s.kind,
            favorite: s.favorite,
            tags: s.tags,
        }
    }
}

/// One field of an item view; concealed values are `None` until revealed.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FieldRow {
    /// Label.
    pub label: String,
    /// Plain value (`None` when concealed).
    pub value: Option<String>,
    /// Hidden until revealed.
    pub concealed: bool,
    /// Section heading ("" for the first block).
    pub section: String,
}

/// An item as its screen shows it (no secret values).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
#[allow(clippy::struct_excessive_bools)] // what the screen shows, flag by flag
pub struct ItemView {
    /// Id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Category.
    pub kind: String,
    /// Username, if any.
    pub username: Option<String>,
    /// Websites.
    pub urls: Vec<String>,
    /// A password is stored (reveal with `reveal_password`).
    pub has_password: bool,
    /// Notes are stored (reveal with `reveal_notes`).
    pub has_notes: bool,
    /// A one-time password is set (codes with `totp`).
    pub has_totp: bool,
    /// Other fields.
    pub fields: Vec<FieldRow>,
    /// The site of the login's passkey, if it has one.
    pub passkey_site: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Starred.
    pub favorite: bool,
    /// In the Archive.
    pub archived: bool,
    /// Attached files.
    pub attachments: Vec<AttachmentRow>,
    /// When it was last changed (ms since epoch).
    pub updated_at: i64,
    /// Passwords it had before the current one.
    pub previous_passwords: u64,
}

/// A file attached to an item.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AttachmentRow {
    /// Id.
    pub id: String,
    /// File name.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Media type ("" when unknown).
    pub content_type: String,
    /// The bytes are on this device.
    pub available: bool,
}

/// A one-time password code.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Code {
    /// The digits.
    pub code: String,
    /// Seconds it stays valid.
    pub remaining: u64,
    /// Seconds per code.
    pub period: u64,
}

/// [`SecretKeyStore`] on top of the app's [`KeyKeeper`].
struct Keeper(Arc<dyn KeyKeeper>);

impl SecretKeyStore for Keeper {
    fn load(&self) -> password365_app::Result<Option<AccountSecretKey>> {
        self.0
            .load()
            .map(|text| AccountSecretKey::parse(&text).map_err(AppError::from))
            .transpose()
    }

    fn save(&self, key: &AccountSecretKey) -> password365_app::Result<()> {
        if self.0.load().is_some() {
            return Err(AppError::Internal("a Secret Key is already stored".into()));
        }
        if self.0.save(key.to_display_string()) {
            Ok(())
        } else {
            Err(AppError::Internal(
                "the Keychain refused the Secret Key".into(),
            ))
        }
    }

    fn remove(&self) -> password365_app::Result<()> {
        self.0.remove();
        Ok(())
    }
}

/// The vault on this device.
#[derive(uniffi::Object)]
pub struct Password365 {
    app: Mutex<AppService>,
    join: pairing::JoinSlot,
    host: hosting::HostSlot,
}

impl Password365 {
    fn app(&self) -> MutexGuard<'_, AppService> {
        // A panic while holding the lock leaves the service usable (it only
        // holds the session); recover rather than poison every later call.
        self.app
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// With explicit key-derivation parameters (tests use fast ones).
    fn with_kdf(data_dir: &str, keeper: Arc<dyn KeyKeeper>, kdf: KdfParams) -> Arc<Self> {
        let dir = PathBuf::from(data_dir);
        let first_start = !dir.join("settings.json").exists();
        let config = AppConfig {
            store_path: dir.join("vault"),
            kdf,
            auto_lock_ms: 0,
            settings_path: Some(dir.join("settings.json")),
        };
        let mut app = AppService::new(config, Box::new(Keeper(keeper)));
        if first_start {
            let mut settings = app.settings();
            settings.auto_lock_minutes = security::MOBILE_AUTO_LOCK_MINUTES;
            let _ = app.update_settings(settings);
        }
        Arc::new(Self {
            app: Mutex::new(app),
            join: pairing::JoinSlot::default(),
            host: hosting::HostSlot::default(),
        })
    }
}

#[uniffi::export]
impl Password365 {
    /// Opens (or prepares) the vault kept in `data_dir`.
    #[uniffi::constructor]
    pub fn new(data_dir: String, keeper: Arc<dyn KeyKeeper>) -> Arc<Self> {
        Self::with_kdf(&data_dir, keeper, KdfParams::MOBILE)
    }

    /// Account and lock state. A vault left open longer than the lock
    /// setting is locked first: what asks (the app coming back, autofill)
    /// never sees it open past its time.
    pub fn status(&self) -> Status {
        self.status_at(password365_model::Timestamp::now())
    }

    /// Creates the account; returns the Secret Key for the Emergency Kit.
    pub fn create_account(&self, password: String, confirmation: String) -> Result<String> {
        Ok(self
            .app()
            .create_account(password.into(), &SecretString::from(confirmation))?)
    }

    /// Unlocks with the master password (and the Secret Key when this device
    /// does not have it yet).
    pub fn unlock(&self, password: String, secret_key: Option<String>) -> Result<()> {
        Ok(self.app().unlock(password.into(), secret_key.as_deref())?)
    }

    /// Locks (the session's keys are wiped).
    pub fn lock(&self) {
        self.app().lock();
    }

    /// Items whose title, username, website or tag contains `query`.
    pub fn items(&self, query: String) -> Result<Vec<ItemRow>> {
        Ok(self
            .app()
            .items(&query, None)?
            .into_iter()
            .map(ItemRow::from)
            .collect())
    }

    /// One item, without secret values.
    pub fn item(&self, id: String) -> Result<ItemView> {
        let item = self.app().item(&id)?;
        Ok(ItemView {
            id: item.summary.id,
            title: item.summary.title,
            kind: item.summary.kind,
            username: item.summary.username,
            urls: item.summary.urls,
            has_password: item.has_password,
            has_notes: item.has_notes,
            has_totp: item.has_totp,
            fields: item
                .fields
                .into_iter()
                .map(|f| FieldRow {
                    label: f.label,
                    value: f.value,
                    concealed: f.concealed,
                    section: f.section,
                })
                .collect(),
            passkey_site: item.passkey.map(|p| p.rp_id),
            tags: item.summary.tags,
            favorite: item.summary.favorite,
            archived: item.summary.archived,
            updated_at: item.summary.updated_at,
            previous_passwords: item.password_history_count as u64,
            attachments: item
                .attachments
                .into_iter()
                .map(|a| AttachmentRow {
                    id: a.id,
                    name: a.name,
                    size: a.size,
                    content_type: a.content_type,
                    available: a.available,
                })
                .collect(),
        })
    }

    /// The password of item `id`.
    pub fn reveal_password(&self, id: String) -> Result<Option<String>> {
        Ok(self.app().reveal(&id, SecretField::Password)?)
    }

    /// The notes of item `id`.
    pub fn reveal_notes(&self, id: String) -> Result<Option<String>> {
        Ok(self.app().reveal(&id, SecretField::Notes)?)
    }

    /// A concealed field's value.
    pub fn reveal_field(&self, id: String, label: String) -> Result<Option<String>> {
        Ok(self.app().reveal_field(&id, &label)?)
    }

    /// Creates a login; returns its id. Empty values are left out.
    pub fn create_login(
        &self,
        title: String,
        username: String,
        password: String,
        website: String,
    ) -> Result<String> {
        let some = |s: String| (!s.trim().is_empty()).then_some(s);
        let input = password365_app::item_input::ItemInput {
            title,
            username: some(username),
            password: some(password).map(SecretString::from),
            urls: some(website).into_iter().collect(),
            ..password365_app::item_input::ItemInput::default()
        };
        Ok(self.app().create_item(&input)?.summary.id)
    }

    /// The current one-time password of item `id`.
    pub fn totp(&self, id: String) -> Result<Option<Code>> {
        Ok(self.app().totp_code(&id, None)?.map(|c| Code {
            code: c.code,
            remaining: c.remaining,
            period: c.period,
        }))
    }
}

#[cfg(test)]
mod tests;
