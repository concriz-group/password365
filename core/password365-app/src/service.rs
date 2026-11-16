//! The application service: lock state + every user-facing operation.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use password365_crypto::kdf::KdfParams;
use password365_crypto::secret::SecretString;
use password365_crypto::secret_key::AccountSecretKey;
use password365_generator::PasswordRecipe;
use password365_generator::entropy::strength_label;
use password365_model::{Item, ItemId, ItemKind, ItemOverview, Timestamp, VaultId};
use password365_vault::private_file::create_private_file;
use password365_vault::{Credentials, DEFAULT_VAULT_NAME, ItemSummary, Session, Store};

use crate::auto_lock::{AutoLock, DEFAULT_TIMEOUT_MS};
use crate::dto::{
    AppStatus, CategoryDto, GenerateRequest, GeneratedDto, ImportSummary, ItemDto, ItemSummaryDto,
    PasswordUpdateDto, SecretField, TagDto, TotpCodeDto, VaultDto,
};
use crate::item_input::ItemInput;
use crate::password_policy::validate_new_master_password;
use crate::secret_key_store::SecretKeyStore;
use crate::settings::Settings;
use crate::{AppError, Result, transfer};

/// Static configuration of the service.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Location of the encrypted store.
    pub store_path: PathBuf,
    /// Argon2id parameters for new accounts / password changes.
    pub kdf: KdfParams,
    /// Idle time before auto-lock (ms); ≤ 0 disables. Overridden by saved settings.
    pub auto_lock_ms: i64,
    /// Where user preferences are persisted (`None` = keep in memory only).
    pub settings_path: Option<PathBuf>,
}

impl AppConfig {
    /// Desktop defaults for a store at `store_path`.
    #[must_use]
    pub fn desktop(store_path: PathBuf) -> Self {
        Self {
            store_path,
            kdf: KdfParams::DESKTOP,
            auto_lock_ms: DEFAULT_TIMEOUT_MS,
            settings_path: None,
        }
    }

    /// Persists preferences at `path`.
    #[must_use]
    pub fn with_settings(mut self, path: PathBuf) -> Self {
        self.settings_path = Some(path);
        self
    }
}

/// Holds the (optional) unlocked session and implements every operation.
pub struct AppService {
    pub(crate) config: AppConfig,
    pub(crate) keys: Box<dyn SecretKeyStore>,
    session: Option<Session>,
    auto_lock: AutoLock,
    pub(crate) backup_log: crate::backup::BackupLog,
    pub(crate) profile_choice: crate::profile::ProfileChoice,
    settings: Settings,
    attempts: crate::attempts::Attempts,
    /// Unlocks so far (permissions given "until the vault locks" belong to one).
    unlocks: u64,
}

impl AppService {
    /// Creates a locked service.
    #[must_use]
    pub fn new(mut config: AppConfig, keys: Box<dyn SecretKeyStore>) -> Self {
        let settings = match config.settings_path.as_deref().and_then(Settings::load) {
            Some(saved) => {
                config.auto_lock_ms = saved.auto_lock_ms();
                saved
            }
            None => Settings {
                auto_lock_minutes: u32::try_from(config.auto_lock_ms / 60_000).unwrap_or(0),
                ..Settings::default()
            },
        };
        let auto_lock = AutoLock::new(config.auto_lock_ms, Timestamp::now());
        let backup_log = crate::backup::load_log(config.settings_path.as_deref());
        let profile_choice = crate::profile::load(config.settings_path.as_deref());
        let config_settings_path = config.settings_path.clone();
        Self {
            config,
            keys,
            session: None,
            auto_lock,
            backup_log,
            profile_choice,
            settings,
            attempts: crate::attempts::load(config_settings_path.as_deref()),
            unlocks: 0,
        }
    }

    // ---------------------------------------------------------------- account

    /// Whether an account exists and whether it is unlocked.
    #[must_use]
    pub fn status(&self) -> AppStatus {
        AppStatus {
            initialized: self.config.store_path.is_file(),
            unlocked: self.session.is_some(),
        }
    }

    /// Creates the account, stores the Secret Key on this device, and leaves
    /// the vault unlocked. Returns the Secret Key text for the Emergency Kit.
    pub fn create_account(
        &mut self,
        password: SecretString,
        confirmation: &SecretString,
    ) -> Result<String> {
        if self.status().initialized {
            return Err(AppError::AlreadyInitialized);
        }
        validate_new_master_password(&password, confirmation)?;
        let secret_key = AccountSecretKey::generate()?;
        self.keys.save(&secret_key)?;
        let credentials = Credentials {
            password,
            secret_key,
        };
        match Store::create(&self.config.store_path, &credentials, self.config.kdf) {
            Ok(session) => {
                self.start_session(session);
                Ok(credentials.secret_key.to_display_string())
            }
            Err(e) => {
                self.keys.remove()?;
                Err(e.into())
            }
        }
    }

    /// Unlocks with the master password. `secret_key` is only needed when this
    /// device does not have it stored yet; a typed key is then remembered.
    fn unlock_unguarded(&mut self, password: SecretString, secret_key: Option<&str>) -> Result<()> {
        let stored = self.keys.load()?;
        let secret_key = match (stored, secret_key) {
            (Some(key), _) => key,
            (None, Some(typed)) => AccountSecretKey::parse(typed)?,
            (None, None) => return Err(AppError::SecretKeyRequired),
        };
        let session = Store::open(&self.config.store_path)?.unlock(&Credentials {
            password,
            secret_key: secret_key.clone(),
        })?;
        if self.keys.load()?.is_none() {
            self.keys.save(&secret_key)?;
        }
        self.start_session(session);
        self.purge_expired_deletions(Timestamp::now())
    }

    /// Runs a master-password check, making the next try wait after wrong ones.
    pub(crate) fn guarded<T>(&mut self, check: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let now = Timestamp::now().as_millis();
        self.attempts.check(now)?;
        let result = check(self);
        match &result {
            // Never deletes anything: only makes the next try wait longer.
            Err(AppError::WrongCredentials) => self.attempts.failed(now),
            Ok(_) if self.attempts.failures() > 0 => self.attempts.succeeded(),
            _ => return result,
        }
        crate::attempts::save(self.config.settings_path.as_deref(), &self.attempts);
        result
    }

    /// Unlocks with the master password (and the Secret Key on a new device).
    /// Wrong passwords make the next try wait longer each time.
    pub fn unlock(&mut self, password: SecretString, secret_key: Option<&str>) -> Result<()> {
        self.guarded(|s| s.unlock_unguarded(password, secret_key))
    }

    /// Changes the master password (the current one is checked like an unlock).
    pub fn change_master_password(
        &mut self,
        current: SecretString,
        new: SecretString,
        confirmation: &SecretString,
    ) -> Result<()> {
        self.guarded(|s| s.change_master_password_unguarded(current, new, confirmation))
    }

    /// Takes the master password set on another device (checked like an unlock).
    pub fn adopt_master_password(&mut self, new: SecretString) -> Result<()> {
        self.guarded(|s| s.adopt_master_password_unguarded(new))
    }

    /// The Secret Key, after checking the master password like an unlock.
    pub fn reveal_secret_key(&mut self, password: SecretString) -> Result<String> {
        self.guarded(|s| s.reveal_secret_key_unguarded(password))
    }

    /// After an unlock: the security settings sealed in the vault win over
    /// the settings file (which any program can edit); a vault without them
    /// gets the current ones.
    fn adopt_sealed_settings(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let sealed = session
            .app_data()
            .and_then(|d| serde_json::from_slice::<crate::settings::SecuritySettings>(d).ok());
        match sealed {
            Some(sealed) if sealed != crate::settings::SecuritySettings::of(&self.settings) => {
                let restored = sealed.applied_to(&self.settings);
                if let Some(path) = &self.config.settings_path {
                    let _ = restored.save(path);
                }
                self.config.auto_lock_ms = restored.auto_lock_ms();
                self.auto_lock = AutoLock::new(self.config.auto_lock_ms, Timestamp::now());
                self.settings = restored;
            }
            Some(_) => {}
            None => {
                let _ = seal_security(session, &self.settings);
            }
        }
    }

    /// Which unlock is in effect (a new number after every unlock); 0 while
    /// locked. Permissions given until the vault locks remember this number.
    #[must_use]
    pub const fn unlock_number(&self) -> u64 {
        if self.session.is_some() {
            self.unlocks
        } else {
            0
        }
    }

    /// Locks immediately, wiping keys from memory.
    pub fn lock(&mut self) {
        self.session = None;
    }

    /// Locks if the idle timeout has passed. Returns `true` if it locked now.
    pub fn check_auto_lock(&mut self, now: Timestamp) -> bool {
        if self.session.is_some() && self.auto_lock.is_expired(now) {
            self.lock();
            return true;
        }
        false
    }

    /// Changes the master password after verifying the current one.
    fn change_master_password_unguarded(
        &mut self,
        current: SecretString,
        new: SecretString,
        confirmation: &SecretString,
    ) -> Result<()> {
        self.session()?;
        validate_new_master_password(&new, confirmation)?;
        let secret_key = self.verify_master_password(current)?;
        let kdf = self.config.kdf;
        self.session()?.change_master_password(
            &Credentials {
                password: new,
                secret_key,
            },
            kdf,
        )?;
        Ok(())
    }

    /// A master password change made on another device that this one does
    /// not use yet.
    pub fn pending_password_change(&mut self) -> Result<Option<PasswordUpdateDto>> {
        let session = self.session()?;
        let Some(change) = session.pending_password_change() else {
            return Ok(None);
        };
        let device_name = session
            .peer(change.changed_by())
            .map_or_else(|_| "another device".to_owned(), |p| p.name.clone());
        Ok(Some(PasswordUpdateDto {
            device_name,
            changed_at: change.changed_at().as_millis(),
        }))
    }

    /// Starts using the master password set on another device.
    fn adopt_master_password_unguarded(&mut self, new: SecretString) -> Result<()> {
        let secret_key = self.keys.load()?.ok_or(AppError::NotInitialized)?;
        let kdf = self.config.kdf;
        self.session()?.adopt_password_change(
            &Credentials {
                password: new,
                secret_key,
            },
            kdf,
        )?;
        Ok(())
    }

    /// The Secret Key, to print a new Emergency Kit. Asks for the master
    /// password again even though the vault is unlocked.
    fn reveal_secret_key_unguarded(&mut self, password: SecretString) -> Result<String> {
        self.session()?;
        Ok(self.verify_master_password(password)?.to_display_string())
    }

    /// Checks `password` with an independent unlock; returns the Secret Key.
    fn verify_master_password(&self, password: SecretString) -> Result<AccountSecretKey> {
        let secret_key = self.keys.load()?.ok_or(AppError::NotInitialized)?;
        Store::open(&self.config.store_path)?.unlock(&Credentials {
            password,
            secret_key: secret_key.clone(),
        })?;
        Ok(secret_key)
    }

    // --------------------------------------------------------------- settings

    /// Current preferences.
    #[must_use]
    pub fn settings(&self) -> Settings {
        self.settings.clone()
    }

    /// Validates, persists and applies new preferences.
    pub fn update_settings(&mut self, settings: Settings) -> Result<()> {
        settings.validate()?;
        if let Some(path) = &self.config.settings_path {
            settings.save(path)?;
        }
        if let Some(session) = self.session.as_mut() {
            seal_security(session, &settings)?;
        }
        self.config.auto_lock_ms = settings.auto_lock_ms();
        self.auto_lock = AutoLock::new(self.config.auto_lock_ms, Timestamp::now());
        self.settings = settings;
        Ok(())
    }

    // ----------------------------------------------------------------- vaults

    /// All vaults with item counts.
    pub fn vaults(&mut self) -> Result<Vec<VaultDto>> {
        let session = self.session()?;
        Ok(session
            .vaults()
            .into_iter()
            .map(|v| {
                let active = session.items_in(v.id).into_iter().filter(|s| active(s));
                VaultDto::new(v, active.count())
            })
            .collect())
    }

    /// Creates a vault.
    pub fn create_vault(&mut self, name: &str) -> Result<VaultDto> {
        let session = self.session()?;
        let id = session.create_vault(name)?;
        Ok(VaultDto::new(session.vault(id)?, 0))
    }

    /// Renames a vault.
    pub fn rename_vault(&mut self, id: &str, name: &str) -> Result<()> {
        let id = parse_id::<VaultId>(id)?;
        Ok(self.session()?.rename_vault(id, name)?)
    }

    /// Deletes an empty vault.
    pub fn delete_vault(&mut self, id: &str) -> Result<()> {
        let id = parse_id::<VaultId>(id)?;
        Ok(self.session()?.delete_vault(id)?)
    }

    // ------------------------------------------------------------------ items

    /// Items matching `query`, optionally in one vault.
    pub fn items(&mut self, query: &str, vault_id: Option<&str>) -> Result<Vec<ItemSummaryDto>> {
        let vault = vault_id.map(parse_id::<VaultId>).transpose()?;
        self.listed(query, |s| {
            active(s) && vault.is_none_or(|v| s.vault_id == v)
        })
    }

    /// Items in the Archive matching `query`.
    pub fn archived_items(&mut self, query: &str) -> Result<Vec<ItemSummaryDto>> {
        self.listed(query, |s| s.overview.archived)
    }

    fn listed(
        &mut self,
        query: &str,
        keep: impl Fn(&ItemSummary) -> bool,
    ) -> Result<Vec<ItemSummaryDto>> {
        let session = self.session()?;
        Ok(session
            .search(query)
            .into_iter()
            .filter(|s| keep(s))
            .map(|s| ItemSummaryDto {
                needs_review: session.needs_review(s.id),
                ..ItemSummaryDto::from(s)
            })
            .collect())
    }

    /// Moves an item to the Archive, or back to the item list.
    pub fn set_archived(&mut self, id: &str, archived: bool) -> Result<()> {
        self.update_overview(id, |o| o.archived = archived)
    }

    /// Every tag in use, with how many items carry it.
    pub fn tags(&mut self) -> Result<Vec<TagDto>> {
        Ok(TagDto::collect(
            self.session()?.search("").into_iter().filter(|s| active(s)),
        ))
    }

    /// Adds an item to Favorites or takes it out.
    pub fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<()> {
        self.update_overview(id, |o| o.favorite = favorite)
    }

    /// Moves an item to another vault (the item menu's Move…).
    pub fn move_item(&mut self, id: &str, vault_id: &str) -> Result<ItemDto> {
        let target = parse_id(vault_id)?;
        self.session()?.vault(target)?;
        let mut item = self.load_item(id)?;
        if item.vault_id != target {
            item.move_to(target, Timestamp::now());
            self.session()?.save_item(&item)?;
        }
        Ok(ItemDto::from(&item))
    }

    /// A copy of an item, titled `title`, in the same vault (the item menu's
    /// Duplicate). Every field is copied; the password history is not.
    pub fn duplicate_item(&mut self, id: &str, title: &str) -> Result<ItemDto> {
        let item = self.load_item(id)?;
        let mut overview = item.overview.clone();
        overview.title = ItemOverview::new(title)?.title;
        overview.favorite = false;
        let details = password365_model::ItemDetails {
            password_history: password365_model::PasswordHistory::default(),
            ..item.details.clone()
        };
        let copy = self
            .session()?
            .add_item(item.vault_id, item.kind, overview, details)?;
        self.item(&copy.to_string())
    }

    /// Applies `change` to an item's overview and saves it if anything changed.
    fn update_overview(&mut self, id: &str, change: impl FnOnce(&mut ItemOverview)) -> Result<()> {
        let mut item = self.load_item(id)?;
        let before = item.overview.clone();
        change(&mut item.overview);
        if item.overview != before {
            item.touch(Timestamp::now());
            self.session()?.save_item(&item)?;
        }
        Ok(())
    }

    /// One item (without secrets).
    pub fn item(&mut self, id: &str) -> Result<ItemDto> {
        let item = self.load_item(id)?;
        let mut dto = ItemDto::from(&item);
        dto.summary.needs_review = self.session()?.needs_review(item.id);
        for (shown, attachment) in dto.attachments.iter_mut().zip(&item.details.attachments) {
            shown.available = self.session()?.has_attachment(attachment.id)?;
        }
        Ok(dto)
    }

    /// The user has reviewed an item flagged after a concurrent password change.
    pub fn mark_reviewed(&mut self, id: &str) -> Result<()> {
        let id = parse_id::<ItemId>(id)?;
        Ok(self.session()?.mark_reviewed(id)?)
    }

    /// Reveals one secret field of an item.
    pub fn reveal(&mut self, id: &str, field: SecretField) -> Result<Option<String>> {
        let item = self.load_item(id)?;
        let secret = match field {
            SecretField::Password => item.details.password.as_ref(),
            SecretField::Notes => item.details.notes.as_ref(),
            SecretField::Totp => item.details.totp.as_ref(),
        };
        Ok(secret.map(|s| s.expose().to_owned()))
    }

    /// Reveals one custom or category field of an item.
    pub fn reveal_field(&mut self, id: &str, label: &str) -> Result<Option<String>> {
        let item = self.load_item(id)?;
        Ok(item
            .details
            .custom_fields
            .iter()
            .find(|f| f.label == label && !crate::field_layout::is_internal(&f.label))
            .map(|f| f.value.expose().to_owned()))
    }

    /// The categories and their fields.
    #[must_use]
    pub fn categories() -> Vec<CategoryDto> {
        CategoryDto::all()
    }

    /// The current one-time password of an item, or of its one-time
    /// password field `label` (the secret never leaves).
    pub fn totp_code(&mut self, id: &str, label: Option<&str>) -> Result<Option<TotpCodeDto>> {
        let item = self.load_item(id)?;
        let secret = match label {
            None => item.details.totp.as_ref(),
            Some(label) => crate::field_layout::describe(item.kind, &item.details.custom_fields)
                .into_iter()
                .find(|(f, m)| {
                    f.label == label
                        && m.input == password365_model::item_template::FieldInput::Totp
                })
                .map(|(f, _)| &f.value),
        };
        let Some(secret) = secret else {
            return Ok(None);
        };
        let totp = password365_crypto::totp::Totp::parse(secret.expose())?;
        let now = u64::try_from(Timestamp::now().as_millis() / 1000).unwrap_or_default();
        Ok(Some(totp.code_at(now).into()))
    }

    /// Creates an item.
    pub fn create_item(&mut self, input: &ItemInput) -> Result<ItemDto> {
        let kind = match input.kind.as_deref() {
            Some(k) => ItemKind::from_str(k).map_err(AppError::Validation)?,
            None => ItemKind::Login,
        };
        let overview = input.overview()?;
        let vault = self.vault_or_default(input.vault_id.as_deref())?;
        let id = self.session()?.add_item(
            vault,
            kind,
            overview,
            crate::ssh_agent::with_derived(kind, input.new_details(kind)?),
        )?;
        self.item(&id.to_string())
    }

    /// Updates an item. `expected_version` must match the stored version.
    pub fn update_item(
        &mut self,
        id: &str,
        expected_version: u64,
        input: &ItemInput,
    ) -> Result<ItemDto> {
        let mut item = self.load_item(id)?;
        if item.version != expected_version {
            return Err(AppError::Conflict);
        }
        let now = Timestamp::now();
        if let Some(target) = input.vault_id.as_deref() {
            item.move_to(parse_id(target)?, now);
        }
        input.apply_to(&mut item, now)?;
        item.details = crate::ssh_agent::with_derived(item.kind, std::mem::take(&mut item.details));
        self.session()?.save_item(&item)?;
        Ok(ItemDto::from(&item))
    }

    /// Moves an item to Recently Deleted.
    pub fn delete_item(&mut self, id: &str) -> Result<()> {
        let id = parse_id::<ItemId>(id)?;
        Ok(self.session()?.delete_item(id)?)
    }

    // ------------------------------------------------------------- generator

    /// Generates a password, passphrase or PIN (works while locked).
    pub fn generate(&mut self, request: &GenerateRequest) -> Result<GeneratedDto> {
        let (secret, bits) = match request {
            GenerateRequest::Password(r) => (r.generate()?, r.entropy_bits()),
            GenerateRequest::Passphrase(r) => (r.generate()?, r.entropy_bits()),
            GenerateRequest::Pin { digits } => {
                let r = PasswordRecipe::pin(*digits);
                (r.generate()?, r.entropy_bits())
            }
        };
        Ok(GeneratedDto {
            value: secret.expose().to_owned(),
            bits,
            strength: strength_label(bits),
        })
    }

    // -------------------------------------------------------- import / export

    /// Imports a browser CSV file.
    pub fn import_csv(
        &mut self,
        path: &Path,
        vault_id: Option<&str>,
        update: bool,
    ) -> Result<ImportSummary> {
        let file = File::open(path)
            .map_err(|e| AppError::Io(format!("cannot open {}: {e}", path.display())))?;
        let vault = self.vault_or_default(vault_id)?;
        transfer::import_csv(self.session()?, file, vault, update, Timestamp::now())
    }

    /// Checks the master password again (like an unlock: wrong ones make
    /// the next try wait) before something leaves the vault in clear.
    pub(crate) fn confirm_master_password(&mut self, password: SecretString) -> Result<()> {
        self.guarded(|s| {
            s.session()?;
            s.verify_master_password(password).map(|_| ())
        })
    }

    /// Exports logins to a new owner-only Chrome CSV file (PLAINTEXT), once
    /// the master password is typed again.
    pub fn export_csv(
        &mut self,
        path: &Path,
        vault_id: Option<&str>,
        password: SecretString,
    ) -> Result<usize> {
        self.confirm_master_password(password)?;
        let vault = vault_id.map(parse_id::<VaultId>).transpose()?;
        let session = self.session()?;
        if let Some(v) = vault {
            session.vault(v)?;
        }
        let file = create_private_file(path)
            .map_err(|e| AppError::Io(format!("cannot create {}: {e}", path.display())))?;
        transfer::export_csv(session, file, vault)
    }

    // ---------------------------------------------------------------- helpers

    pub(crate) fn start_session(&mut self, session: Session) {
        self.session = Some(session);
        self.unlocks += 1;
        self.adopt_sealed_settings();
        self.auto_lock = AutoLock::new(self.config.auto_lock_ms, Timestamp::now());
    }

    /// The unlocked session without touching the idle timer.
    pub(crate) fn unlocked_session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// The unlocked session; enforces auto-lock. Calls do not count as
    /// activity: the browser extension, the SSH agent, timers and codes that
    /// refresh call in without the user (see [`Self::user_active`]).
    pub(crate) fn session(&mut self) -> Result<&mut Session> {
        self.check_auto_lock(Timestamp::now());
        self.session.as_mut().ok_or(AppError::Locked)
    }

    /// The user typed, clicked or scrolled in the app's window: postpones the
    /// idle lock (or locks now if the idle time is already over).
    pub fn user_active(&mut self, now: Timestamp) {
        if !self.check_auto_lock(now) {
            self.auto_lock.touch(now);
        }
    }

    pub(crate) fn load_item(&mut self, id: &str) -> Result<Item> {
        let id = parse_id::<ItemId>(id)?;
        Ok(self.session()?.item(id)?)
    }

    pub(crate) fn vault_or_default(&mut self, id: Option<&str>) -> Result<VaultId> {
        let session = self.session()?;
        match id {
            Some(id) => {
                let id = parse_id::<VaultId>(id)?;
                session.vault(id)?;
                Ok(id)
            }
            None => session
                .vault_by_name(DEFAULT_VAULT_NAME)
                .or_else(|| session.vaults().into_iter().next())
                .map(|v| v.id)
                .ok_or(AppError::NotFound("vault")),
        }
    }
}

/// Keeps `settings`' security part sealed in the vault.
fn seal_security(session: &mut Session, settings: &Settings) -> Result<()> {
    let json = serde_json::to_vec(&crate::settings::SecuritySettings::of(settings))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(session.set_app_data(&json)?)
}

pub(crate) fn parse_id<T: FromStr>(id: &str) -> Result<T> {
    id.parse()
        .map_err(|_| AppError::Validation(format!("invalid id '{id}'")))
}

#[cfg(test)]
#[path = "service_tests.rs"]
pub(crate) mod tests;

/// Shown in the item list (not in the Archive).
pub(crate) fn active(summary: &ItemSummary) -> bool {
    !summary.overview.archived
}
