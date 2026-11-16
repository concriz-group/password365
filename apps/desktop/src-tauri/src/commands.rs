//! IPC commands. Each one is a thin adapter over `password365_app::AppService`;
//! no business logic lives here.

use std::path::PathBuf;

use password365_app::Result;
use password365_app::backup::BackupInfo;
use password365_app::dto::{
    AppStatus, GenerateRequest, GeneratedDto, ImportSummary, ItemDto, ItemSummaryDto, SecretField,
    VaultDto,
};
use password365_app::item_input::ItemInput;
use password365_app::password_policy::{self, PasswordStrength};
use password365_app::recently_deleted::DeletedItemDto;
use password365_app::settings::Settings;
use password365_crypto::secret::SecretString;
use tauri::State;

use crate::state::SharedState;

#[tauri::command]
pub fn status(state: State<'_, SharedState>) -> Result<AppStatus> {
    state.with(|s| Ok(s.status()))
}

#[tauri::command]
pub fn create_account(
    state: State<'_, SharedState>,
    password: SecretString,
    confirmation: SecretString,
) -> Result<String> {
    state.with(|s| s.create_account(password, &confirmation))
}

#[tauri::command]
pub fn unlock(
    state: State<'_, SharedState>,
    password: SecretString,
    secret_key: Option<String>,
) -> Result<()> {
    state.with(|s| s.unlock(password, secret_key.as_deref()))
}

/// Strength of a candidate master password (live meter while typing).
#[tauri::command]
pub fn password_strength(password: SecretString) -> PasswordStrength {
    password_policy::password_strength(&password)
}

/// The user typed, clicked or scrolled in a window: postpones the idle lock.
/// (Nothing else counts: the extension, the SSH agent and timers do not.)
#[tauri::command]
pub fn user_active(state: State<'_, SharedState>) -> Result<()> {
    state.with(|s| {
        s.user_active(password365_model::Timestamp::now());
        Ok(())
    })
}

#[tauri::command]
pub fn lock(state: State<'_, SharedState>) -> Result<()> {
    state.with(|s| {
        s.lock();
        Ok(())
    })
}

#[tauri::command]
pub fn change_master_password(
    state: State<'_, SharedState>,
    current: SecretString,
    new_password: SecretString,
    confirmation: SecretString,
) -> Result<()> {
    state.with(|s| s.change_master_password(current, new_password, &confirmation))
}

#[tauri::command]
pub fn pending_password_change(
    state: State<'_, SharedState>,
) -> Result<Option<password365_app::dto::PasswordUpdateDto>> {
    state.with(password365_app::AppService::pending_password_change)
}

#[tauri::command]
pub fn adopt_master_password(state: State<'_, SharedState>, password: SecretString) -> Result<()> {
    state.with(|s| s.adopt_master_password(password))
}

#[tauri::command]
pub fn reveal_secret_key(state: State<'_, SharedState>, password: SecretString) -> Result<String> {
    state.with(|s| s.reveal_secret_key(password))
}

#[tauri::command]
pub fn vaults(state: State<'_, SharedState>) -> Result<Vec<VaultDto>> {
    state.with(password365_app::AppService::vaults)
}

#[tauri::command]
pub fn create_vault(state: State<'_, SharedState>, name: String) -> Result<VaultDto> {
    state.with(|s| s.create_vault(&name))
}

#[tauri::command]
pub fn rename_vault(state: State<'_, SharedState>, id: String, name: String) -> Result<()> {
    state.with(|s| s.rename_vault(&id, &name))
}

#[tauri::command]
pub fn delete_vault(state: State<'_, SharedState>, id: String) -> Result<()> {
    state.with(|s| s.delete_vault(&id))
}

#[tauri::command]
pub fn items(
    state: State<'_, SharedState>,
    query: String,
    vault_id: Option<String>,
) -> Result<Vec<ItemSummaryDto>> {
    state.with(|s| s.items(&query, vault_id.as_deref()))
}

#[tauri::command]
pub fn item(state: State<'_, SharedState>, id: String) -> Result<ItemDto> {
    state.with(|s| s.item(&id))
}

#[tauri::command]
pub fn totp_code(
    state: State<'_, SharedState>,
    id: String,
    label: Option<String>,
) -> Result<Option<password365_app::dto::TotpCodeDto>> {
    state.with(|s| s.totp_code(&id, label.as_deref()))
}

#[tauri::command]
pub fn reveal(
    state: State<'_, SharedState>,
    id: String,
    field: SecretField,
) -> Result<Option<String>> {
    state.with(|s| s.reveal(&id, field))
}

#[tauri::command]
pub fn create_item(state: State<'_, SharedState>, input: ItemInput) -> Result<ItemDto> {
    state.with(|s| s.create_item(&input))
}

#[tauri::command]
pub fn update_item(
    state: State<'_, SharedState>,
    id: String,
    version: u64,
    input: ItemInput,
) -> Result<ItemDto> {
    state.with(|s| s.update_item(&id, version, &input))
}

#[tauri::command]
pub fn delete_item(state: State<'_, SharedState>, id: String) -> Result<()> {
    state.with(|s| s.delete_item(&id))
}

#[tauri::command]
pub fn tags(state: State<'_, SharedState>) -> Result<Vec<password365_app::dto::TagDto>> {
    state.with(password365_app::AppService::tags)
}

#[tauri::command]
pub fn profile(state: State<'_, SharedState>) -> Result<password365_app::profile::ProfileDto> {
    state.with(password365_app::AppService::profile)
}

#[tauri::command]
pub fn set_profile(
    state: State<'_, SharedState>,
    kind: password365_app::profile::ProfileKind,
    id: Option<String>,
) -> Result<()> {
    state.with(|s| s.set_profile(kind, id.as_deref()))
}

#[tauri::command]
pub fn auto_type(app: tauri::AppHandle, state: State<'_, SharedState>, id: String) -> Result<()> {
    crate::auto_typer::type_into_previous_window(&app, &state, &id)
}

#[tauri::command]
pub fn watchtower(
    state: State<'_, SharedState>,
) -> Result<password365_app::watchtower::WatchtowerDto> {
    state.with(password365_app::AppService::watchtower)
}

#[tauri::command]
pub fn categories() -> Vec<password365_app::dto::CategoryDto> {
    password365_app::AppService::categories()
}

#[tauri::command]
pub fn reveal_field(
    state: State<'_, SharedState>,
    id: String,
    label: String,
) -> Result<Option<String>> {
    state.with(|s| s.reveal_field(&id, &label))
}

#[tauri::command]
pub fn archived_items(state: State<'_, SharedState>, query: String) -> Result<Vec<ItemSummaryDto>> {
    state.with(|s| s.archived_items(&query))
}

#[tauri::command]
pub fn set_archived(state: State<'_, SharedState>, id: String, archived: bool) -> Result<()> {
    state.with(|s| s.set_archived(&id, archived))
}

/// SSH Key items and whether the agent can use them, plus where it listens.
#[tauri::command]
pub fn ssh_keys(state: State<'_, SharedState>) -> Result<SshKeysDto> {
    Ok(SshKeysDto {
        address: crate::ssh_agent::address(),
        keys: state.with(password365_app::AppService::ssh_keys)?,
    })
}

/// [`ssh_keys`]'s answer.
#[derive(serde::Serialize)]
pub struct SshKeysDto {
    address: String,
    keys: Vec<password365_app::ssh_agent::SshKeyDto>,
}

/// Logins whose password appeared in data breaches (haveibeenpwned.com; only
/// hash prefixes leave the device). The vault is read quickly; the network
/// part runs in the background so the window stays responsive.
#[tauri::command]
pub async fn breached_passwords(
    state: State<'_, SharedState>,
) -> Result<Vec<password365_app::breach::BreachedDto>> {
    let candidates = state.with(password365_app::AppService::breach_candidates)?;
    crate::sync_commands::blocking(move || {
        password365_app::breach::check(candidates, &password365_app::breach::Hibp)
    })
    .await
}

/// Attaches the file at `path` to item `id`.
#[tauri::command]
pub fn attach_file(
    state: State<'_, SharedState>,
    id: String,
    path: PathBuf,
) -> Result<password365_app::attachments::AttachmentDto> {
    state.with(|s| s.attach_file(&id, &path))
}

/// Saves a decrypted copy of an attachment where the user chose.
#[tauri::command]
pub fn save_attachment(
    state: State<'_, SharedState>,
    id: String,
    attachment_id: String,
    path: PathBuf,
) -> Result<()> {
    state.with(|s| s.save_attachment(&id, &attachment_id, &path))
}

/// Opens an attachment with the system's program (temporary decrypted copy).
#[tauri::command]
pub fn open_attachment(
    app: tauri::AppHandle,
    state: State<'_, SharedState>,
    id: String,
    attachment_id: String,
) -> Result<()> {
    let (name, bytes) = state.with(|s| {
        Ok((
            s.attachment_name(&id, &attachment_id)?,
            s.attachment_bytes(&id, &attachment_id)?,
        ))
    })?;
    crate::attachment_files::open(&app, &name, &bytes)
}

/// Imports a CXF (credential exchange) file, passkeys included.
#[tauri::command]
pub fn import_cxf(
    state: State<'_, SharedState>,
    path: PathBuf,
    vault_id: Option<String>,
) -> Result<ImportSummary> {
    state.with(|s| s.import_cxf(&path, vault_id.as_deref()))
}

/// Exports items to a CXF (credential exchange) file, plaintext.
#[tauri::command]
pub fn export_cxf(
    state: State<'_, SharedState>,
    path: PathBuf,
    vault_id: Option<String>,
    password: SecretString,
) -> Result<usize> {
    state.with(|s| s.export_cxf(&path, vault_id.as_deref(), password))
}

/// Removes the passkey from login `id`.
#[tauri::command]
pub fn remove_passkey(state: State<'_, SharedState>, id: String) -> Result<()> {
    state.with(|s| s.remove_passkey(&id))
}

/// Removes an attachment from item `id`.
#[tauri::command]
pub fn remove_attachment(
    state: State<'_, SharedState>,
    id: String,
    attachment_id: String,
) -> Result<()> {
    state.with(|s| s.remove_attachment(&id, &attachment_id))
}

/// Every item's own icon, by item id.
#[tauri::command]
pub fn item_icons(
    state: State<'_, SharedState>,
) -> Result<std::collections::BTreeMap<String, String>> {
    state.with(password365_app::AppService::item_icons)
}

/// The QR code (SVG) that joins a Wireless Router item's network, if any.
#[tauri::command]
pub fn wifi_qr(state: State<'_, SharedState>, id: String) -> Result<Option<String>> {
    state
        .with(|s| s.wifi_join_text(&id))?
        .map(|text| crate::sync_commands::qr_svg(&text))
        .transpose()
}

#[tauri::command]
pub fn move_item(state: State<'_, SharedState>, id: String, vault_id: String) -> Result<ItemDto> {
    state.with(|s| s.move_item(&id, &vault_id))
}

#[tauri::command]
pub fn duplicate_item(state: State<'_, SharedState>, id: String, title: String) -> Result<ItemDto> {
    state.with(|s| s.duplicate_item(&id, &title))
}

#[tauri::command]
pub fn set_favorite(state: State<'_, SharedState>, id: String, favorite: bool) -> Result<()> {
    state.with(|s| s.set_favorite(&id, favorite))
}

#[tauri::command]
pub fn mark_reviewed(state: State<'_, SharedState>, id: String) -> Result<()> {
    state.with(|s| s.mark_reviewed(&id))
}

#[tauri::command]
pub fn deleted_items(state: State<'_, SharedState>) -> Result<Vec<DeletedItemDto>> {
    state.with(password365_app::AppService::deleted_items)
}

#[tauri::command]
pub fn restore_item(state: State<'_, SharedState>, id: String) -> Result<()> {
    state.with(|s| s.restore_item(&id))
}

#[tauri::command]
pub fn delete_permanently(state: State<'_, SharedState>, id: String) -> Result<()> {
    state.with(|s| s.delete_permanently(&id))
}

#[tauri::command]
pub fn generate(state: State<'_, SharedState>, request: GenerateRequest) -> Result<GeneratedDto> {
    state.with(|s| s.generate(&request))
}

#[tauri::command]
pub fn import_csv(
    state: State<'_, SharedState>,
    path: PathBuf,
    vault_id: Option<String>,
    update: bool,
) -> Result<ImportSummary> {
    state.with(|s| s.import_csv(&path, vault_id.as_deref(), update))
}

/// Imports 1Password's full export (`.1pux`) into a vault.
#[tauri::command]
pub fn import_1pux(
    state: State<'_, SharedState>,
    path: PathBuf,
    vault_id: Option<String>,
) -> Result<ImportSummary> {
    state.with(|s| s.import_1pux(&path, vault_id.as_deref()))
}

/// Imports a `KeePass` / `KeePassXC` database (`.kdbx`) opened with its password.
#[tauri::command]
pub fn import_kdbx(
    state: State<'_, SharedState>,
    path: PathBuf,
    password: SecretString,
    key_file: Option<PathBuf>,
    vault_id: Option<String>,
) -> Result<ImportSummary> {
    state.with(|s| s.import_kdbx(&path, &password, key_file.as_deref(), vault_id.as_deref()))
}

/// Imports Bitwarden's JSON export (unencrypted) into a vault.
#[tauri::command]
pub fn import_bitwarden(
    state: State<'_, SharedState>,
    path: PathBuf,
    vault_id: Option<String>,
) -> Result<ImportSummary> {
    state.with(|s| s.import_bitwarden(&path, vault_id.as_deref()))
}

/// Writes an encrypted backup of everything to `path`.
#[tauri::command]
pub fn export_backup(state: State<'_, SharedState>, path: PathBuf) -> Result<BackupInfo> {
    state.with(|s| s.export_backup(&path))
}

/// Last backup and whether a new one is due.
#[tauri::command]
pub fn backup_info(state: State<'_, SharedState>) -> Result<BackupInfo> {
    state.with(|s| Ok(s.backup_info()))
}

/// First run on a new device: restore a backup.
#[tauri::command]
pub fn restore_backup(
    state: State<'_, SharedState>,
    path: PathBuf,
    password: SecretString,
    secret_key: String,
) -> Result<()> {
    state.with(|s| s.restore_backup(&path, password, &secret_key))
}

#[tauri::command]
pub fn export_csv(
    state: State<'_, SharedState>,
    path: PathBuf,
    vault_id: Option<String>,
    password: SecretString,
) -> Result<usize> {
    state.with(|s| s.export_csv(&path, vault_id.as_deref(), password))
}

#[tauri::command]
pub fn settings(state: State<'_, SharedState>) -> Result<Settings> {
    state.with(|s| Ok(s.settings()))
}

#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    state: State<'_, SharedState>,
    settings: Settings,
) -> Result<()> {
    crate::shortcuts::validate(&settings)?;
    let applied = settings.clone();
    let start_at_login = settings.start_at_login;
    let ssh_agent = settings.ssh_agent;
    let before = state.with(|s| {
        let before = s.settings().start_at_login;
        s.update_settings(settings)?;
        Ok(before)
    })?;
    if before != start_at_login {
        crate::autostart::apply(&app, start_at_login)?;
    }
    if ssh_agent {
        crate::ssh_agent::start(&app);
    }
    crate::shortcuts::apply(&app, &applied);
    Ok(())
}

/// Opens the native print dialog for the current window (Emergency Kit).
#[tauri::command]
pub fn print_page(window: tauri::WebviewWindow) -> Result<()> {
    window
        .print()
        .map_err(|e| password365_app::AppError::Internal(e.to_string()))
}
