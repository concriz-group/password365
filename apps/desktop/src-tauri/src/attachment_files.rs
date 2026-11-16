//! Opening an attachment: a decrypted copy in this app's cache folder
//! (owner-only), opened with the system's default program. Copies are
//! deleted after a few minutes and whenever the app starts.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use password365_app::AppError;
use tauri::{AppHandle, Manager, Runtime};

/// How long an opened copy stays.
const KEEP: Duration = Duration::from_secs(10 * 60);

fn folder<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_cache_dir()
        .ok()
        .map(|d| d.join("opened-attachments"))
}

/// Deletes opened copies older than [`KEEP`] (all of them with `everything`).
pub fn clean<R: Runtime>(app: &AppHandle<R>, everything: bool) {
    let Some(folder) = folder(app) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|at| SystemTime::now().duration_since(at).ok())
            .is_some_and(|age| age > KEEP);
        if everything || old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

pub(crate) fn open_with_system(path: &std::ffi::OsStr) -> std::io::Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(path);
        c
    } else if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]).arg(path);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    command.spawn().map(|_| ())
}

/// Writes `bytes` as `name` in a fresh private folder and opens it.
pub fn open<R: Runtime>(app: &AppHandle<R>, name: &str, bytes: &[u8]) -> Result<(), AppError> {
    use std::io::Write as _;
    clean(app, false);
    let base = folder(app).ok_or_else(|| AppError::Internal("no cache folder".into()))?;
    let safe: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '\0') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let path = base.join(uuid_like()).join(safe);
    let fail = |e: std::io::Error| AppError::Io(format!("cannot open the file: {e}"));
    password365_vault::private_file::create_private_file(&path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(fail)?;
    open_with_system(path.as_os_str()).map_err(fail)
}

fn uuid_like() -> String {
    password365_model::AttachmentId::new().to_string()
}
