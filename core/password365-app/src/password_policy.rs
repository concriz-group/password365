//! Rules for choosing a master password: a minimum length and a minimum
//! strength estimated by zxcvbn (dictionary words, keyboard patterns, dates,
//! repeats, l33t substitutions…), like 1Password's strength meter.

use password365_crypto::secret::SecretString;
use serde::Serialize;

use crate::{AppError, Result};

/// Minimum master password length (characters).
pub const MIN_PASSWORD_LEN: usize = 10;
/// Minimum zxcvbn score (0–4) for a master password: 3 = "good".
pub const MIN_STRENGTH_SCORE: u8 = 3;
/// zxcvbn gets slow on very long inputs; anything longer is judged on this prefix.
const ANALYZED_CHARS: usize = 100;
/// Words that make a password guessable here.
const CONTEXT_WORDS: &[&str] = &["password365", "password", "master", "vault"];

/// Strength of a candidate master password (no secret inside — safe to show).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PasswordStrength {
    /// 0 (very weak) … 4 (very strong).
    pub score: u8,
    /// `very_weak`, `weak`, `fair`, `good` or `strong`.
    pub label: &'static str,
    /// Characters still missing to reach the minimum length.
    pub missing_chars: usize,
    /// Main problem, if any ("This is a very common password.").
    pub warning: Option<String>,
    /// How to improve it.
    pub suggestions: Vec<String>,
    /// `true` when the password may be used as a master password.
    pub acceptable: bool,
}

const LABELS: [&str; 5] = ["very_weak", "weak", "fair", "good", "strong"];

/// Estimates the strength of a candidate master password.
#[must_use]
pub fn password_strength(password: &SecretString) -> PasswordStrength {
    let text = password.expose();
    let chars = text.chars().count();
    let analyzed: String = text.chars().take(ANALYZED_CHARS).collect();
    let entropy = zxcvbn::zxcvbn(&analyzed, CONTEXT_WORDS);
    let score = u8::from(entropy.score());
    let feedback = entropy.feedback();
    let missing_chars = MIN_PASSWORD_LEN.saturating_sub(chars);
    PasswordStrength {
        score,
        label: LABELS[usize::from(score.min(4))],
        missing_chars,
        warning: feedback
            .and_then(zxcvbn::feedback::Feedback::warning)
            .map(|w| w.to_string()),
        suggestions: feedback
            .map(|f| f.suggestions().iter().map(ToString::to_string).collect())
            .unwrap_or_default(),
        acceptable: missing_chars == 0 && score >= MIN_STRENGTH_SCORE,
    }
}

/// Checks a new master password (and its confirmation).
pub fn validate_new_master_password(
    password: &SecretString,
    confirmation: &SecretString,
) -> Result<()> {
    let strength = password_strength(password);
    if strength.missing_chars > 0 {
        return Err(AppError::Validation(format!(
            "master password must have at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    if !strength.acceptable {
        let reason = strength
            .warning
            .unwrap_or_else(|| "it is too easy to guess".into());
        return Err(AppError::Validation(format!(
            "master password is too weak: {reason}"
        )));
    }
    if password != confirmation {
        return Err(AppError::Validation("passwords do not match".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(pw: &str) -> Result<()> {
        let pw: SecretString = pw.into();
        validate_new_master_password(&pw, &pw.clone())
    }

    #[test]
    fn accepts_long_strong_matching_password() {
        assert!(check("long enough password").is_ok());
        assert!(check("cavalo-nuvem-trilho-azul-7").is_ok());
    }

    #[test]
    fn rejects_short_password() {
        let err = check("short").unwrap_err();
        assert!(err.to_string().contains("at least 10"));
    }

    #[test]
    fn rejects_long_but_guessable_passwords() {
        for weak in [
            "1234567890",
            "password123",
            "qwertyuiop",
            "aaaaaaaaaaaa",
            "password3652026",
        ] {
            let err = check(weak).unwrap_err();
            assert!(err.to_string().contains("too weak"), "{weak}: {err}");
        }
    }

    #[test]
    fn counts_characters_not_bytes() {
        let s = password_strength(&"éàü".into());
        assert_eq!(s.missing_chars, 7);
    }

    #[test]
    fn rejects_mismatch() {
        let err = validate_new_master_password(
            &"long enough password".into(),
            &"long enough passw0rd".into(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("do not match"));
    }

    #[test]
    fn strength_report_explains_weak_passwords() {
        let s = password_strength(&"password".into());
        assert_eq!((s.score, s.label, s.acceptable), (0, "very_weak", false));
        assert_eq!(s.missing_chars, 2);
        assert!(s.warning.is_some());
        assert!(!s.suggestions.is_empty());

        let strong = password_strength(&"correct horse battery staple 42".into());
        assert_eq!((strong.label, strong.acceptable), ("strong", true));
        assert!(strong.warning.is_none());
    }

    #[test]
    fn very_long_passwords_are_analyzed_quickly() {
        let long: SecretString = "x9!Kq".repeat(1000).into();
        assert!(password_strength(&long).missing_chars == 0);
    }
}
