//! Password365 desktop app: a Tauri window over `password365_app::AppService`.

mod attachment_files;
mod auto_lock;
mod auto_typer;
mod autostart;
mod bridge;
mod browser_host;
mod clipboard;
mod commands;
mod help;
mod key_bypass;
mod keychain;
mod legacy;
mod pairing_slots;
mod quick_access;
mod screen_lock;
mod shortcuts;
mod ssh_agent;
mod state;
#[cfg(test)]
mod state_registration_tests;
mod sync_commands;
mod sync_service;
mod updates;

use std::path::Path;
use std::sync::Arc;

use password365_app::secret_key_store::{
    FileSecretKeyStore, MigratingSecretKeyStore, SecretKeyStore, SecretSlot, SlotSecretKeyStore,
};
use password365_app::{AppConfig, AppService};
use tauri::{Emitter, Manager};

use crate::state::{AppState, SharedState};

/// Store file name inside the app data directory.
const STORE_FILE: &str = "default.qlvault";
/// Preferences file name inside the app data directory.
const SETTINGS_FILE: &str = "settings.json";
/// Fallback Secret Key file (only used when no OS keychain is available, and
/// migrated into the keychain when one is).
const SECRET_KEY_FILE: &str = "secret-key";
/// Present when the keychain entry belongs to another build and is ignored (see `key_bypass`).
const KEYCHAIN_BYPASS_MARKER: &str = "secret-key.keychain-bypassed";

/// Where the Secret Key lives: the OS keychain when available, otherwise an
/// owner-only file. A key found in an older place (the key file, or the
/// keychain entry of the app's former name) is moved into the keychain.
fn key_store<S: SecretSlot + 'static>(
    key_dir: &Path,
    keychain: Option<S>,
    legacy_keychain: Option<S>,
) -> Box<dyn SecretKeyStore> {
    let file: Box<dyn SecretKeyStore> =
        Box::new(FileSecretKeyStore::new(key_dir.join(SECRET_KEY_FILE)));
    let Some(slot) = keychain else {
        return file;
    };
    let older = match legacy_keychain {
        Some(old) => Box::new(MigratingSecretKeyStore::new(
            Box::new(SlotSecretKeyStore::new(old)),
            file,
        )),
        None => file,
    };
    Box::new(key_bypass::BypassingStore::new(
        Box::new(MigratingSecretKeyStore::new(
            Box::new(SlotSecretKeyStore::new(slot)),
            older,
        )),
        key_dir.join(SECRET_KEY_FILE),
        key_dir.join(KEYCHAIN_BYPASS_MARKER),
    ))
}

/// Builds the service for the given directories.
fn build_service(data_dir: &Path, keys: Box<dyn SecretKeyStore>) -> AppService {
    AppService::new(
        AppConfig::desktop(data_dir.join(STORE_FILE)).with_settings(data_dir.join(SETTINGS_FILE)),
        keys,
    )
}

/// Opens the vault service, starts the background services and registers shortcuts.
fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let paths = app.path();
    let (data_dir, key_dir) = (paths.app_data_dir()?, paths.app_local_data_dir()?);
    legacy::adopt_beside(&data_dir, STORE_FILE)?;
    legacy::adopt_beside(&key_dir, SECRET_KEY_FILE)?;
    let keys = key_store(
        &key_dir,
        keychain::KeychainSlot::secret_key(),
        keychain::KeychainSlot::legacy_secret_key(),
    );
    let state: SharedState = Arc::new(AppState::new(build_service(&data_dir, keys)));
    let emitter = app.handle().clone();
    let notify = move || {
        let _ = emitter.emit(state::STATUS_EVENT, ());
    };
    auto_lock::spawn(Arc::clone(&state), notify.clone());
    attachment_files::spawn_cleaner(app.handle().clone(), Arc::clone(&state));
    bridge::start(Arc::clone(&state), notify);
    let synced = app.handle().clone();
    sync_service::start(Arc::clone(&state), move || {
        let _ = synced.emit(sync_service::SYNC_EVENT, ());
    });
    app.manage(state);
    app.manage(ssh_agent::SharedApprovals::default());
    ssh_agent::start_if_enabled(app.handle());
    attachment_files::clean(app.handle(), true);
    app.manage(shortcuts::Registered::default());
    let settings = app
        .state::<SharedState>()
        .with(|s| Ok(s.settings()))
        .unwrap_or_default();
    shortcuts::apply(app.handle(), &settings);
    app.manage(pairing_slots::SharedSlots::default());
    app.manage(updates::Found::default());
    updates::start(app.handle());
    // The extension's helper ships next to the app: tell the browsers where it is.
    std::thread::spawn(browser_host::register);
    Ok(())
}

/// Starts the desktop application.
///
/// # Panics
/// Panics if the Tauri runtime cannot start (no display, broken install).
pub fn run() {
    tauri::Builder::default()
        // Reopens the window at the size and place it was closed with.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(shortcuts::plugin())
        .plugin(autostart::plugin())
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            help::open_help,
            updates::update_check,
            updates::update_install,
            commands::status,
            commands::erase_vault,
            commands::create_account,
            commands::unlock,
            commands::lock,
            commands::user_active,
            commands::change_master_password,
            commands::password_strength,
            commands::vaults,
            commands::create_vault,
            commands::rename_vault,
            commands::delete_vault,
            commands::items,
            commands::item,
            commands::reveal,
            commands::totp_code,
            commands::reveal_secret_key,
            commands::save_emergency_kit,
            commands::pending_password_change,
            commands::adopt_master_password,
            commands::create_item,
            commands::update_item,
            commands::delete_item,
            commands::mark_reviewed,
            commands::tags,
            commands::set_favorite,
            commands::archived_items,
            commands::set_archived,
            commands::move_item,
            commands::wifi_qr,
            commands::item_icons,
            commands::attach_file,
            commands::save_attachment,
            commands::open_attachment,
            commands::remove_attachment,
            commands::remove_passkey,
            commands::export_cxf,
            commands::import_cxf,
            clipboard::copy_secret,
            commands::ssh_keys,
            commands::breached_passwords,
            commands::import_1pux,
            commands::import_bitwarden,
            commands::import_kdbx,
            ssh_agent::ssh_answer,
            quick_access::hide_quick_access,
            quick_access::open_in_app,
            commands::duplicate_item,
            commands::categories,
            commands::watchtower,
            commands::auto_type,
            commands::profile,
            commands::set_profile,
            commands::reveal_field,
            commands::deleted_items,
            commands::restore_item,
            commands::delete_permanently,
            commands::generate,
            commands::import_csv,
            commands::export_csv,
            commands::export_backup,
            commands::backup_info,
            commands::restore_backup,
            commands::settings,
            commands::update_settings,
            commands::print_page,
            sync_commands::this_device,
            sync_commands::rename_this_device,
            sync_commands::devices,
            sync_commands::remove_device,
            sync_commands::sync_device,
            sync_commands::export_sync_file,
            sync_commands::import_sync_file,
            sync_commands::invite_device,
            sync_commands::wait_for_new_device,
            sync_commands::approve_new_device,
            sync_commands::cancel_pairing,
            sync_commands::join_with_code,
            sync_commands::join_show_code,
            sync_commands::connect_new_device,
            sync_commands::join_accept_scan,
            sync_commands::join_wait,
            sync_commands::join_complete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Password365");
}

#[cfg(test)]
mod tests {
    use password365_app::secret_key_store::MemorySlot;

    use super::*;

    const PW: &str = "long enough password";

    #[test]
    fn a_key_in_the_former_apps_keychain_entry_moves_to_the_new_one() {
        let (data, keys) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let old = MemorySlot::default();
        let mut first = build_service(data.path(), key_store(keys.path(), Some(old.clone()), None));
        first.create_account(PW.into(), &PW.into()).unwrap();
        let new = MemorySlot::default();
        let mut renamed = build_service(
            data.path(),
            key_store(keys.path(), Some(new.clone()), Some(old.clone())),
        );
        renamed.unlock(PW.into(), None).unwrap();
        assert!(new.get().unwrap().is_some(), "moved to the new entry");
        assert!(old.get().unwrap().is_none());
    }

    #[test]
    fn without_keychain_the_key_goes_to_a_file_apart_from_the_store() {
        let data = tempfile::tempdir().unwrap();
        let keys = tempfile::tempdir().unwrap();
        let mut service = build_service(
            data.path(),
            key_store::<MemorySlot>(keys.path(), None, None),
        );
        service.create_account(PW.into(), &PW.into()).ok();
        assert!(data.path().join(STORE_FILE).exists());
        assert!(keys.path().join(SECRET_KEY_FILE).exists());
    }
}
