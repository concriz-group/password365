//! Noticing that the computer's screen was locked (Settings › Security ›
//! Lock when the computer locks or sleeps), using what each system already
//! exposes — no native code:
//! - macOS: `ioreg` reports `IOConsoleLocked`;
//! - Windows: the lock screen is the `LogonUI.exe` process;
//! - Linux (systemd): `loginctl` reports the session's `LockedHint`.

use std::process::{Command, Output};

/// Whether the screen is locked now; `None` when it cannot be told.
#[must_use]
pub fn is_locked() -> Option<bool> {
    if cfg!(target_os = "macos") {
        let out = run(Command::new("ioreg").args(["-n", "Root", "-d1", "-a"]))?;
        macos_locked(&String::from_utf8_lossy(&out.stdout))
    } else if cfg!(windows) {
        let out = run(Command::new("tasklist").args(["/FI", "IMAGENAME eq LogonUI.exe", "/NH"]))?;
        Some(windows_locked(&String::from_utf8_lossy(&out.stdout)))
    } else {
        let session = std::env::var("XDG_SESSION_ID").ok()?;
        let out =
            run(Command::new("loginctl").args(["show-session", &session, "-p", "LockedHint"]))?;
        linux_locked(&String::from_utf8_lossy(&out.stdout))
    }
}

fn run(command: &mut Command) -> Option<Output> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.output().ok().filter(|o| o.status.success())
}

/// `<key>IOConsoleLocked</key>` followed by `<true/>` or `<false/>`.
fn macos_locked(plist: &str) -> Option<bool> {
    let after = plist
        .split("<key>IOConsoleLocked</key>")
        .nth(1)?
        .trim_start();
    if after.starts_with("<true/>") {
        Some(true)
    } else if after.starts_with("<false/>") {
        Some(false)
    } else {
        None
    }
}

fn windows_locked(tasklist: &str) -> bool {
    tasklist.to_ascii_lowercase().contains("logonui.exe")
}

fn linux_locked(output: &str) -> Option<bool> {
    match output.trim() {
        "LockedHint=yes" => Some(true),
        "LockedHint=no" => Some(false),
        _ => None,
    }
}

/// Turns repeated readings into "it was just locked" (once per lock).
#[derive(Debug, Default)]
pub struct ScreenWatch {
    was_locked: bool,
}

impl ScreenWatch {
    /// `true` the first time a reading says locked after it was not.
    pub fn just_locked(&mut self, reading: Option<bool>) -> bool {
        let Some(locked) = reading else {
            return false;
        };
        let now = locked && !self.was_locked;
        self.was_locked = locked;
        now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_each_system() {
        assert_eq!(
            macos_locked("<dict>\n\t<key>IOConsoleLocked</key>\n\t<true/>"),
            Some(true)
        );
        assert_eq!(
            macos_locked("<key>IOConsoleLocked</key><false/>"),
            Some(false)
        );
        assert_eq!(macos_locked("<key>Other</key>"), None);
        assert_eq!(macos_locked("<key>IOConsoleLocked</key><string/>"), None);
        assert!(windows_locked(
            "LogonUI.exe                  1234 Console    1     40,000 K"
        ));
        assert!(!windows_locked(
            "INFO: No tasks are running which match the specified criteria."
        ));
        assert_eq!(linux_locked("LockedHint=yes\n"), Some(true));
        assert_eq!(linux_locked("LockedHint=no"), Some(false));
        assert_eq!(linux_locked(""), None);
    }

    #[test]
    fn reports_each_lock_once() {
        let mut watch = ScreenWatch::default();
        assert!(!watch.just_locked(Some(false)));
        assert!(watch.just_locked(Some(true)));
        assert!(!watch.just_locked(Some(true)), "still the same lock");
        assert!(!watch.just_locked(None), "unknown changes nothing");
        assert!(!watch.just_locked(Some(false)));
        assert!(watch.just_locked(Some(true)), "a new lock");
    }

    #[test]
    fn this_computer_answers() {
        // Tests run with the screen unlocked; machines without a desktop session answer None.
        assert_ne!(is_locked(), Some(true));
    }
}
