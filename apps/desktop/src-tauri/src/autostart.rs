//! Settings › General › Start at login: registers the app with the system
//! (a launch agent on macOS, the Run key on Windows, an autostart entry on
//! Linux) only when the user turns it on.

use password365_app::AppError;
use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// The autostart plugin (nothing is registered until [`apply`] is called).
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None)
}

/// Turns starting at login on or off.
pub fn apply<R: Runtime>(app: &AppHandle<R>, on: bool) -> Result<(), AppError> {
    let launcher = app.autolaunch();
    let result = if on {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|e| AppError::Internal(format!("start at login: {e}")))
}
