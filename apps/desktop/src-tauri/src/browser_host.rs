//! Registers the browser extension's helper (`password365-host`, shipped next
//! to the app) with the browsers installed for this user, every time the app
//! starts — so the extension can reach the app without any setup, and the
//! registration follows the app when it moves or updates (like 1Password).
//!
//! macOS and Linux: a JSON manifest in each browser's `NativeMessagingHosts`
//! folder. Windows: the manifests in the app's data folder, and one registry
//! key per browser under HKCU (no administrator rights). Only browsers whose
//! folders exist are touched, and a manifest is rewritten only if it changed.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The native-messaging name the extension connects to.
pub const NAME: &str = "app.password365.host";
/// The extension's ids (Chromium and Firefox), shared with the extension's build.
const IDS: &str = include_str!("../../../../extension/scripts/extension-key.json");

/// The helper's file name.
pub const HOST_FILE: &str = if cfg!(windows) {
    "password365-host.exe"
} else {
    "password365-host"
};

/// Which manifest a browser reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Chromium,
    Firefox,
}

fn ids() -> (String, String) {
    let parsed: Value = serde_json::from_str(IDS).unwrap_or_default();
    let get = |key: &str| parsed[key].as_str().unwrap_or_default().to_owned();
    (get("id"), get("firefox_id"))
}

/// The manifest a browser of `family` needs to start `host`.
pub fn manifest(family: Family, host: &Path) -> Value {
    let (chromium, firefox) = ids();
    let mut m = json!({
        "name": NAME,
        "description": "Password365 browser helper",
        "path": host,
        "type": "stdio",
    });
    match family {
        Family::Chromium => {
            m["allowed_origins"] = json!([format!("chrome-extension://{chromium}/")]);
        }
        Family::Firefox => {
            m["allowed_extensions"] = json!([firefox]);
        }
    }
    m
}

/// Folders (macOS, Linux) where browsers look for manifests, for the
/// browsers whose own folder exists under `home`.
pub fn unix_targets(home: &Path, mac: bool) -> Vec<(PathBuf, Family)> {
    let support = if mac {
        home.join("Library/Application Support")
    } else {
        home.join(".config")
    };
    let chromium: &[&str] = if mac {
        &[
            "Google/Chrome",
            "Google/Chrome Beta",
            "Chromium",
            "BraveSoftware/Brave-Browser",
            "Microsoft Edge",
        ]
    } else {
        &[
            "google-chrome",
            "chromium",
            "BraveSoftware/Brave-Browser",
            "microsoft-edge",
        ]
    };
    let mut out: Vec<(PathBuf, Family)> = chromium
        .iter()
        .map(|b| support.join(b))
        .filter(|dir| dir.is_dir())
        .map(|dir| (dir.join("NativeMessagingHosts"), Family::Chromium))
        .collect();
    let mozilla = if mac {
        support.join("Mozilla")
    } else {
        home.join(".mozilla")
    };
    if mozilla.is_dir() {
        let folder = if mac {
            "NativeMessagingHosts"
        } else {
            "native-messaging-hosts"
        };
        out.push((mozilla.join(folder), Family::Firefox));
    }
    out
}

/// Writes `manifest` as `<dir>/<NAME>.json` unless it is already there;
/// returns whether it wrote.
pub fn write(dir: &Path, manifest: &Value) -> std::io::Result<bool> {
    let file = dir.join(format!("{NAME}.json"));
    let text = serde_json::to_string_pretty(manifest)? + "\n";
    if std::fs::read_to_string(&file).is_ok_and(|old| old == text) {
        return Ok(false);
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(&file, text)?;
    Ok(true)
}

/// The helper shipped next to the running app, if it is there.
fn shipped_host() -> Option<PathBuf> {
    let host = std::env::current_exe().ok()?.parent()?.join(HOST_FILE);
    host.is_file().then_some(host)
}

/// Registers the helper for every installed browser (errors are only logged:
/// the app works without the extension).
pub fn register() {
    let Some(host) = shipped_host() else {
        return;
    };
    let Some(home) = dirs_home() else {
        return;
    };
    if cfg!(windows) {
        windows::register(&home, &host);
        return;
    }
    for (dir, family) in unix_targets(&home, cfg!(target_os = "macos")) {
        if let Err(e) = write(&dir, &manifest(family, &host)) {
            eprintln!(
                "password365: browser helper not registered in {}: {e}",
                dir.display()
            );
        }
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Windows: manifests under %LOCALAPPDATA%, found through HKCU registry keys.
mod windows {
    use std::path::Path;

    use super::{Family, NAME, manifest, write};

    const KEYS: [(&str, Family); 5] = [
        (
            r"HKCU\Software\Google\Chrome\NativeMessagingHosts",
            Family::Chromium,
        ),
        (
            r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts",
            Family::Chromium,
        ),
        (
            r"HKCU\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts",
            Family::Chromium,
        ),
        (
            r"HKCU\Software\Chromium\NativeMessagingHosts",
            Family::Chromium,
        ),
        (
            r"HKCU\Software\Mozilla\NativeMessagingHosts",
            Family::Firefox,
        ),
    ];

    pub fn register(home: &Path, host: &Path) {
        let base = std::env::var_os("LOCALAPPDATA")
            .map_or_else(|| home.join("AppData/Local"), std::path::PathBuf::from)
            .join("Password365/NativeMessagingHosts");
        for (key, family) in KEYS {
            let dir = base.join(match family {
                Family::Chromium => "chromium",
                Family::Firefox => "firefox",
            });
            let file = dir.join(format!("{NAME}.json"));
            let wrote = write(&dir, &manifest(family, host));
            if wrote.is_err() {
                continue;
            }
            let _ = std::process::Command::new("reg")
                .args([
                    "add",
                    &format!(r"{key}\{NAME}"),
                    "/ve",
                    "/t",
                    "REG_SZ",
                    "/d",
                ])
                .arg(&file)
                .arg("/f")
                .output();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_name_the_shipped_helper_and_only_our_extension() {
        let host = Path::new("/Applications/Password365.app/Contents/MacOS/password365-host");
        let chromium = manifest(Family::Chromium, host);
        assert_eq!(chromium["name"], NAME);
        assert_eq!(chromium["path"], host.display().to_string());
        assert_eq!(chromium["type"], "stdio");
        let origins = chromium["allowed_origins"].as_array().unwrap();
        assert_eq!(origins.len(), 1);
        assert!(
            origins[0]
                .as_str()
                .unwrap()
                .starts_with("chrome-extension://")
        );
        assert!(origins[0].as_str().unwrap().len() > "chrome-extension:///".len());
        let firefox = manifest(Family::Firefox, host);
        assert_eq!(
            firefox["allowed_extensions"],
            json!(["extension@password365.app"])
        );
        assert!(firefox.get("allowed_origins").is_none());
    }

    #[test]
    fn only_installed_browsers_get_a_manifest() {
        let home = tempfile::tempdir().unwrap();
        assert!(
            unix_targets(home.path(), true).is_empty(),
            "no browsers, nothing to write"
        );
        std::fs::create_dir_all(
            home.path()
                .join("Library/Application Support/Google/Chrome"),
        )
        .unwrap();
        std::fs::create_dir_all(home.path().join("Library/Application Support/Mozilla")).unwrap();
        let mac = unix_targets(home.path(), true);
        assert_eq!(
            mac,
            [
                (
                    home.path()
                        .join("Library/Application Support/Google/Chrome/NativeMessagingHosts"),
                    Family::Chromium
                ),
                (
                    home.path()
                        .join("Library/Application Support/Mozilla/NativeMessagingHosts"),
                    Family::Firefox
                ),
            ]
        );
        std::fs::create_dir_all(home.path().join(".config/chromium")).unwrap();
        std::fs::create_dir_all(home.path().join(".mozilla")).unwrap();
        let linux = unix_targets(home.path(), false);
        assert_eq!(
            linux,
            [
                (
                    home.path().join(".config/chromium/NativeMessagingHosts"),
                    Family::Chromium
                ),
                (
                    home.path().join(".mozilla/native-messaging-hosts"),
                    Family::Firefox
                ),
            ]
        );
    }

    #[test]
    fn a_manifest_is_written_once_and_rewritten_when_the_app_moves() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("NativeMessagingHosts");
        let first = manifest(Family::Chromium, Path::new("/old/password365-host"));
        assert!(write(&target, &first).unwrap());
        assert!(!write(&target, &first).unwrap(), "unchanged: not rewritten");
        let moved = manifest(Family::Chromium, Path::new("/new/password365-host"));
        assert!(write(&target, &moved).unwrap());
        let saved: Value = serde_json::from_str(
            &std::fs::read_to_string(target.join(format!("{NAME}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["path"], "/new/password365-host");
    }
}
