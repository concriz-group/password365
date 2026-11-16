//! Opening an attachment: a decrypted copy in this app's cache folder
//! (owner-only), opened with the system's default program. Copies are
//! deleted after a few minutes, when the vault locks and whenever the app
//! starts.
//!
//! Attachments can come from imported files, so they are treated like
//! downloads: programs and scripts are never opened (only saved), and the copy
//! is marked as coming from the internet (macOS quarantine, Windows
//! Mark-of-the-Web) so the system warns before running anything in it. The
//! file is handed to the system without a shell: its name is never parsed as
//! a command.

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

/// Deletes opened copies every few seconds: the old ones, and all of them
/// while the vault is locked.
pub fn spawn_cleaner<R: Runtime>(app: AppHandle<R>, state: std::sync::Arc<crate::state::AppState>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(crate::auto_lock::CHECK_INTERVAL);
            let locked = state.with(|s| Ok(!s.status().unlocked)).unwrap_or(true);
            clean(&app, locked);
        }
    });
}

/// Opens a file or web address with the system's default program, passing it
/// as one argument (no shell: `&`, `|` or quotes in it are just characters).
pub(crate) fn open_with_system(path: &std::ffi::OsStr) -> std::io::Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer.exe"
    } else {
        "xdg-open"
    };
    Command::new(program).arg(path).spawn().map(|_| ())
}

/// File types that run code when opened: never opened from the app.
const PROGRAMS: &[&str] = &[
    "app",
    "appimage",
    "applescript",
    "bash",
    "bat",
    "cmd",
    "com",
    "command",
    "cpl",
    "csh",
    "deb",
    "desktop",
    "dmg",
    "exe",
    "fish",
    "gadget",
    "hta",
    "inf",
    "ins",
    "isp",
    "jar",
    "js",
    "jse",
    "ksh",
    "lnk",
    "mpkg",
    "msc",
    "msi",
    "msp",
    "mst",
    "pif",
    "pkg",
    "pl",
    "ps1",
    "ps1xml",
    "ps2",
    "psc1",
    "psm1",
    "py",
    "pyw",
    "rb",
    "reg",
    "rpm",
    "run",
    "scf",
    "scpt",
    "scr",
    "sct",
    "sh",
    "shb",
    "terminal",
    "tool",
    "url",
    "vb",
    "vbe",
    "vbs",
    "webloc",
    "workflow",
    "ws",
    "wsc",
    "wsf",
    "wsh",
    "zsh",
];

/// Whether `name` is a program or script (by its last extension).
pub(crate) fn is_program(name: &str) -> bool {
    let trimmed = name.trim_end_matches(['.', ' ']);
    std::path::Path::new(trimmed)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PROGRAMS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Marks `path` as downloaded from the internet, so the system checks it
/// before running anything (best effort).
fn mark_as_downloaded(path: &std::path::Path) {
    if cfg!(target_os = "macos") {
        let flags = format!(
            "0081;{:x};Password365;",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        );
        let _ = Command::new("xattr")
            .args(["-w", "com.apple.quarantine", &flags])
            .arg(path)
            .output();
    } else if cfg!(windows) {
        let mut stream = path.as_os_str().to_owned();
        stream.push(":Zone.Identifier");
        let _ = std::fs::write(stream, "[ZoneTransfer]\r\nZoneId=3\r\n");
    }
}

/// Writes `bytes` as `name` in a fresh private folder and opens it.
pub fn open<R: Runtime>(app: &AppHandle<R>, name: &str, bytes: &[u8]) -> Result<(), AppError> {
    use std::io::Write as _;
    if is_program(name) {
        return Err(AppError::Validation(
            "Password365 does not open programs or scripts. Save the file instead, and check it before running it.".into(),
        ));
    }
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
    mark_as_downloaded(&path);
    open_with_system(path.as_os_str()).map_err(fail)
}

fn uuid_like() -> String {
    password365_model::AttachmentId::new().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_and_scripts_are_recognised_whatever_the_case() {
        for name in [
            "setup.exe",
            "run.BAT",
            "x.cmd",
            "a.lnk",
            "tool.jar",
            "s.ps1",
            "m.vbs",
            "Installer.pkg",
            "Install.app",
            "evil.sh",
            "doc.pdf.exe",
            "trick.bat.",
            "trick.cmd ",
        ] {
            assert!(is_program(name), "{name}");
        }
        for name in [
            "report.pdf",
            "photo.JPG",
            "notes.txt",
            "a&calc.pdf",
            "no-extension",
            "exe",
        ] {
            assert!(!is_program(name), "{name}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn opened_copies_are_quarantined() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.pdf");
        std::fs::write(&file, b"x").unwrap();
        mark_as_downloaded(&file);
        let out = Command::new("xattr")
            .args(["-p", "com.apple.quarantine"])
            .arg(&file)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("Password365"));
    }
}
