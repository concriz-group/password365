//! Non-secret user preferences, stored as JSON next to the store.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{AppError, Result};

/// Shortest allowed auto-lock timeout (minutes).
pub const MIN_AUTO_LOCK_MINUTES: u32 = 1;
/// Longest allowed auto-lock timeout (minutes). There is deliberately no "never".
pub const MAX_AUTO_LOCK_MINUTES: u32 = 240;
/// Default auto-lock timeout (minutes).
pub const DEFAULT_AUTO_LOCK_MINUTES: u32 = 10;

/// Allowed clipboard clearing delays (seconds); 0 keeps copied text.
pub const CLIPBOARD_CHOICES: &[u32] = &[0, 30, 90, 180, 300];
/// Interface zoom range (percent).
pub const ZOOM_RANGE: std::ops::RangeInclusive<u32> = 50..=200;

/// Light, dark, or as the system is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Follow the system.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/// How much room rows take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// Roomy rows.
    #[default]
    Comfortable,
    /// Tighter rows.
    Compact,
}

/// The screen the app opens to after unlocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenTo {
    /// All items.
    #[default]
    AllItems,
    /// Favorites.
    Favorites,
    /// Watchtower.
    Watchtower,
}

/// Whether the app looks for a new version on password365.app (a network
/// feature: off until the user answers the first-run question).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChecks {
    /// Not asked yet: no checks until the user chooses.
    #[default]
    Unasked,
    /// Once at start and then daily.
    Automatic,
    /// Only when the user asks (Settings › About).
    Manual,
}

/// User preferences (this device only).
// Independent on/off choices, as the Settings screen shows them: not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Lock after this many idle minutes.
    pub auto_lock_minutes: u32,
    /// Interface language as a BCP 47 tag (`pt-BR`); empty: the system's.
    pub language: String,
    /// Lock when the computer sleeps.
    pub lock_on_sleep: bool,
    /// Clear copied secrets after this many seconds (0: never).
    pub clipboard_clear_seconds: u32,
    /// Light, dark or the system's.
    pub theme: Theme,
    /// Row density.
    pub density: Density,
    /// Interface zoom (percent).
    pub zoom_percent: u32,
    /// Show the categories in the sidebar.
    pub sidebar_categories: bool,
    /// Show the tags in the sidebar.
    pub sidebar_tags: bool,
    /// Vault new items go to (empty: the first one).
    pub default_vault: String,
    /// Screen shown after unlocking.
    pub open_to: OpenTo,
    /// Start the app when the user signs in to the computer.
    pub start_at_login: bool,
    /// Show a Wi-Fi network's QR code without asking (Wireless Router items).
    pub show_wifi_qr: bool,
    /// Serve SSH Key items to ssh and git (Settings › Developer).
    pub ssh_agent: bool,
    /// Ask haveibeenpwned.com about passwords (Settings › Privacy; the only
    /// feature that uses the internet, off unless turned on).
    pub check_breached_passwords: bool,
    /// Global shortcut opening Quick Access (empty: none).
    pub shortcut_quick_access: String,
    /// Global shortcut typing the matching item (empty: none).
    pub shortcut_auto_type: String,
    /// Global shortcut locking Password365 (empty: none).
    pub shortcut_lock: String,
    /// Country whose fields new identities use (`BR`), independent of the
    /// language; empty: the system's region; `ZZ`: the international fields.
    pub region: String,
    /// Looking for new versions (Settings › About).
    pub update_checks: UpdateChecks,
    /// A version the user chose to skip: automatic checks do not announce it
    /// (a newer one, or "Check for updates", still does). Empty: none.
    pub skipped_update: String,
}

/// Quick Access's default shortcut: Shift+Cmd+Space on a Mac, like 1Password.
pub const DEFAULT_QUICK_ACCESS: &str = if cfg!(target_os = "macos") {
    "Shift+Super+Space"
} else {
    "Ctrl+Shift+Space"
};
/// Auto-type's default shortcut, as in `KeePass`.
pub const DEFAULT_AUTO_TYPE: &str = "Ctrl+Alt+A";

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: DEFAULT_AUTO_LOCK_MINUTES,
            language: String::new(),
            lock_on_sleep: true,
            clipboard_clear_seconds: 90,
            theme: Theme::System,
            density: Density::Comfortable,
            zoom_percent: 100,
            sidebar_categories: true,
            sidebar_tags: true,
            default_vault: String::new(),
            open_to: OpenTo::AllItems,
            start_at_login: false,
            show_wifi_qr: true,
            ssh_agent: false,
            check_breached_passwords: false,
            shortcut_quick_access: DEFAULT_QUICK_ACCESS.to_owned(),
            shortcut_auto_type: DEFAULT_AUTO_TYPE.to_owned(),
            shortcut_lock: String::new(),
            region: String::new(),
            update_checks: UpdateChecks::Unasked,
            skipped_update: String::new(),
        }
    }
}

/// `true` for a plausible BCP 47 tag (`en`, `pt-BR`, `zh-Hant-TW`) or empty.
fn is_language_tag(tag: &str) -> bool {
    tag.is_empty()
        || tag.split('-').enumerate().all(|(i, part)| {
            let len_ok = if i == 0 {
                (2..=3).contains(&part.len())
            } else {
                (2..=8).contains(&part.len())
            };
            len_ok && part.chars().all(|c| c.is_ascii_alphanumeric())
        })
}

impl Settings {
    /// Checks that every value is within its allowed range.
    pub fn validate(&self) -> Result<()> {
        if !(MIN_AUTO_LOCK_MINUTES..=MAX_AUTO_LOCK_MINUTES).contains(&self.auto_lock_minutes) {
            return Err(AppError::Validation(format!(
                "auto-lock must be between {MIN_AUTO_LOCK_MINUTES} and {MAX_AUTO_LOCK_MINUTES} minutes"
            )));
        }
        if !CLIPBOARD_CHOICES.contains(&self.clipboard_clear_seconds) {
            return Err(AppError::Validation(format!(
                "clipboard clearing must be one of {CLIPBOARD_CHOICES:?} seconds"
            )));
        }
        if !ZOOM_RANGE.contains(&self.zoom_percent) {
            return Err(AppError::Validation(format!(
                "zoom must be between {} and {} percent",
                ZOOM_RANGE.start(),
                ZOOM_RANGE.end()
            )));
        }
        let shortcuts = [
            &self.shortcut_quick_access,
            &self.shortcut_auto_type,
            &self.shortcut_lock,
        ];
        let used: Vec<String> = shortcuts
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect();
        if (1..used.len()).any(|i| used[..i].contains(&used[i])) {
            return Err(AppError::Validation(
                "two actions cannot share a shortcut".into(),
            ));
        }
        if !(self.region.is_empty()
            || (self.region.len() == 2 && self.region.chars().all(|c| c.is_ascii_uppercase())))
        {
            return Err(AppError::Validation(format!(
                "unknown country \"{}\"",
                self.region
            )));
        }
        if !is_language_tag(&self.language) {
            return Err(AppError::Validation(format!(
                "unknown language \"{}\"",
                self.language
            )));
        }
        Ok(())
    }

    /// Auto-lock timeout in milliseconds.
    #[must_use]
    pub fn auto_lock_ms(&self) -> i64 {
        i64::from(self.auto_lock_minutes) * 60_000
    }

    /// Loads settings; a missing, unreadable or invalid file yields `None`
    /// (preferences are never worth refusing to start over).
    #[must_use]
    pub fn load(path: &Path) -> Option<Self> {
        let settings: Self = crate::json_file::read(path)?;
        settings.validate().ok().map(|()| settings)
    }

    /// Validates and saves atomically (write to a temp file, then rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        crate::json_file::write(path, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_conversion() {
        let s = Settings::default();
        assert_eq!(s.auto_lock_minutes, DEFAULT_AUTO_LOCK_MINUTES);
        assert_eq!(s.auto_lock_ms(), 600_000);
        s.validate().unwrap();
    }

    #[test]
    fn update_checks_are_off_until_the_user_answers() {
        assert_eq!(Settings::default().update_checks, UpdateChecks::Unasked);
        // Settings saved before the option existed load with it unasked.
        let old: Settings = serde_json::from_str(r#"{"auto_lock_minutes": 5}"#).unwrap();
        assert_eq!(old.update_checks, UpdateChecks::Unasked);
        let on: Settings = serde_json::from_str(r#"{"update_checks": "automatic"}"#).unwrap();
        assert_eq!(on.update_checks, UpdateChecks::Automatic);
        assert_eq!(Settings::default().skipped_update, "");
    }

    #[test]
    fn rejects_out_of_range() {
        for minutes in [0, MAX_AUTO_LOCK_MINUTES + 1] {
            let err = Settings {
                auto_lock_minutes: minutes,
                ..Settings::default()
            }
            .validate()
            .unwrap_err();
            assert_eq!(err.code(), "validation");
        }
    }

    #[test]
    fn save_then_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let s = Settings {
            auto_lock_minutes: 3,
            ..Settings::default()
        };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), Some(s.clone()));
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn new_preferences_have_safe_defaults_and_limits() {
        let s: Settings = serde_json::from_str(r#"{"auto_lock_minutes": 5}"#).unwrap();
        assert!(s.lock_on_sleep);
        assert_eq!(s.clipboard_clear_seconds, 90);
        assert_eq!(
            (s.theme, s.density, s.open_to),
            (Theme::System, Density::Comfortable, OpenTo::AllItems)
        );
        for bad in [
            Settings {
                clipboard_clear_seconds: 7,
                ..Settings::default()
            },
            Settings {
                zoom_percent: 10,
                ..Settings::default()
            },
        ] {
            assert_eq!(bad.validate().unwrap_err().code(), "validation");
        }
        let json = serde_json::to_string(&Settings {
            theme: Theme::Dark,
            ..Settings::default()
        })
        .unwrap();
        assert!(json.contains(r#""theme":"dark""#) && json.contains(r#""open_to":"all_items""#));
    }

    #[test]
    fn language_tags() {
        for ok in ["", "en", "pt-BR", "zh-Hant-TW", "ast"] {
            assert!(is_language_tag(ok), "{ok}");
        }
        for bad in ["e", "english-", "pt_BR", "../x", "pt-B"] {
            assert!(!is_language_tag(bad), "{bad}");
        }
        let s = Settings {
            language: "pt BR".into(),
            ..Settings::default()
        };
        assert_eq!(s.validate().unwrap_err().code(), "validation");
    }

    #[test]
    fn save_refuses_invalid() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            Settings {
                auto_lock_minutes: 0,
                ..Settings::default()
            }
            .save(&dir.path().join("s.json"))
            .is_err()
        );
    }

    #[test]
    fn load_tolerates_missing_corrupt_and_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        assert_eq!(Settings::load(&path), None);
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Settings::load(&path), None);
        std::fs::write(&path, r#"{"auto_lock_minutes": 0}"#).unwrap();
        assert_eq!(Settings::load(&path), None);
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(
            Settings::load(&path),
            Some(Settings::default()),
            "missing fields use defaults"
        );
    }
}

/// The settings that protect the vault. A copy is kept sealed in the vault
/// (this device only) and wins over the settings file on unlock, so a
/// program that edits the file cannot loosen them unnoticed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecuritySettings {
    auto_lock_minutes: u32,
    lock_on_sleep: bool,
    clipboard_clear_seconds: u32,
    ssh_agent: bool,
    check_breached_passwords: bool,
    update_checks: UpdateChecks,
}

impl SecuritySettings {
    /// The protecting part of `settings`.
    #[must_use]
    pub fn of(settings: &Settings) -> Self {
        Self {
            auto_lock_minutes: settings.auto_lock_minutes,
            lock_on_sleep: settings.lock_on_sleep,
            clipboard_clear_seconds: settings.clipboard_clear_seconds,
            ssh_agent: settings.ssh_agent,
            check_breached_passwords: settings.check_breached_passwords,
            update_checks: settings.update_checks,
        }
    }

    /// `settings` with these values.
    #[must_use]
    pub fn applied_to(&self, settings: &Settings) -> Settings {
        Settings {
            auto_lock_minutes: self.auto_lock_minutes,
            lock_on_sleep: self.lock_on_sleep,
            clipboard_clear_seconds: self.clipboard_clear_seconds,
            ssh_agent: self.ssh_agent,
            check_breached_passwords: self.check_breached_passwords,
            update_checks: self.update_checks,
            ..settings.clone()
        }
    }
}
