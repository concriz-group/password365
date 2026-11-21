//! Help and feedback (Settings › About): fixed addresses only — the window
//! names a topic, never a URL, so it cannot open anything else.

use password365_app::{AppError, Result};
use serde::Deserialize;

const REPO: &str = "https://github.com/concriz-group/password365";

/// What the user wants to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    /// Report something that does not work.
    Bug,
    /// Suggest a feature or an improvement.
    Idea,
    /// Translate Password365 into a language, or fix a translation.
    Translate,
    /// Report a security problem privately (never in a public issue).
    Security,
    /// Write to the team.
    Email,
}

/// Where each topic goes.
pub fn address(topic: Topic) -> String {
    match topic {
        Topic::Bug => format!("{REPO}/issues/new?template=bug_report.yml"),
        Topic::Idea => format!("{REPO}/issues/new?template=feature_request.yml"),
        Topic::Translate => format!("{REPO}/issues/new?template=translation.yml"),
        Topic::Security => format!("{REPO}/security/advisories/new"),
        Topic::Email => "mailto:hello@password365.app".into(),
    }
}

/// Opens the page (or mail app) for `topic`.
#[tauri::command]
pub fn open_help(topic: Topic) -> Result<()> {
    crate::attachment_files::open_with_system(std::ffi::OsStr::new(&address(topic)))
        .map_err(|e| AppError::Io(format!("Could not open the page: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_has_its_own_fixed_address() {
        let all = [
            Topic::Bug,
            Topic::Idea,
            Topic::Translate,
            Topic::Security,
            Topic::Email,
        ];
        let addresses: Vec<String> = all.iter().map(|t| address(*t)).collect();
        for a in &addresses {
            assert!(a.starts_with(REPO) || a.starts_with("mailto:"), "{a}");
            // `cmd /C start` on Windows would split an address at `&`.
            assert!(!a.contains('&'), "{a}");
        }
        let unique: std::collections::HashSet<_> = addresses.iter().collect();
        assert_eq!(unique.len(), all.len());
        assert!(
            address(Topic::Security).contains("/security/advisories/new"),
            "private, not an issue"
        );
    }

    #[test]
    fn the_window_names_topics_not_urls() {
        let topic: Topic = serde_json::from_str("\"translate\"").unwrap();
        assert_eq!(topic, Topic::Translate);
        assert!(serde_json::from_str::<Topic>("\"https://example.com\"").is_err());
    }
}
