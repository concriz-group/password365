//! New versions of Password365. Looked for on password365.app only with the
//! user's consent (the first-run question, Settings › About), and installed
//! only when the user says so. Every update is signed with Password365's
//! update key: the updater plugin refuses anything else, wherever it comes
//! from (public key and address in `tauri.conf.json`).

use std::sync::Mutex;
use std::time::Duration;

use password365_app::settings::UpdateChecks;
use password365_app::{AppError, Result};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::state::SharedState;

/// Event sent to the window when an automatic check found a new version.
pub const UPDATE_EVENT: &str = "update-available";
/// Automatic checks: shortly after start, then once a day.
const FIRST_CHECK: Duration = Duration::from_secs(30);
const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// The newest update found (kept to install it without asking again).
#[derive(Default)]
pub struct Found(Mutex<Option<Update>>);

/// A newer version, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateInfo {
    /// Its version (`0.2.0`).
    pub version: String,
    /// Release notes, if any.
    pub notes: Option<String>,
}

fn network(e: impl std::fmt::Display) -> AppError {
    AppError::Io(format!("Could not check for updates: {e}"))
}

/// Asks password365.app for a newer version; remembers it for `install`.
async fn check<R: Runtime>(app: &AppHandle<R>) -> Result<Option<UpdateInfo>> {
    let update = app
        .updater()
        .map_err(network)?
        .check()
        .await
        .map_err(network)?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        notes: u.body.clone(),
    });
    *app.state::<Found>()
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = update;
    Ok(info)
}

/// Whether automatic checks are on, and the version the user skipped.
fn automatic<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    app.state::<SharedState>()
        .with(|s| Ok(s.settings()))
        .ok()
        .filter(|s| s.update_checks == UpdateChecks::Automatic)
        .map(|s| s.skipped_update)
}

/// An automatic check announces a version unless the user skipped it.
fn announce(found: &UpdateInfo, skipped: &str) -> bool {
    found.version != skipped
}

/// Starts the automatic checks; each one runs only if the setting allows it
/// at that moment, so turning it off takes effect without a restart.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio_sleep(FIRST_CHECK).await;
        loop {
            if let Some(skipped) = automatic(&app)
                && let Ok(Some(info)) = check(&app).await
                && announce(&info, &skipped)
            {
                let _ = app.emit(UPDATE_EVENT, info);
            }
            tokio_sleep(INTERVAL).await;
        }
    });
}

async fn tokio_sleep(wait: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(wait)).await;
}

/// "Check for updates" (Settings › About).
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>> {
    check(&app).await
}

/// Downloads the update found, installs it and restarts the app.
#[tauri::command]
pub async fn update_install(app: AppHandle, found: State<'_, Found>) -> Result<()> {
    let update = found
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .ok_or_else(|| AppError::Validation("No update to install; check again.".into()))?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| AppError::Io(format!("Could not install the update: {e}")))?;
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skipped_version_is_not_announced_but_a_newer_one_is() {
        let found = |v: &str| UpdateInfo {
            version: v.into(),
            notes: None,
        };
        assert!(!announce(&found("0.2.0"), "0.2.0"));
        assert!(announce(&found("0.3.0"), "0.2.0"));
        assert!(announce(&found("0.2.0"), ""));
    }
}
