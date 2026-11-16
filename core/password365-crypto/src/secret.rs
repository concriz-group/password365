//! Zeroizing string for passwords, notes and other secret text.

use std::fmt;

use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A string whose memory is wiped on drop and which never appears in logs.
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretString(String);

impl SecretString {
    /// Wraps an existing string.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Exposes the secret text. Keep the borrow as short as possible.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Returns `true` if the secret is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl PartialEq for SecretString {
    /// Constant-time comparison (length still leaks, which is acceptable).
    fn eq(&self, other: &Self) -> bool {
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }
}

impl Eq for SecretString {}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_wrapped_value() {
        assert_eq!(SecretString::from("hunter2").expose(), "hunter2");
        assert_eq!(SecretString::new("a".into()).expose(), "a");
        assert_eq!(SecretString::from(String::from("b")).expose(), "b");
    }

    #[test]
    fn debug_is_redacted() {
        assert_eq!(
            format!("{:?}", SecretString::from("hunter2")),
            "SecretString(<redacted>)"
        );
    }

    #[test]
    fn equality_compares_content() {
        assert_eq!(SecretString::from("x"), SecretString::from("x"));
        assert_ne!(SecretString::from("x"), SecretString::from("y"));
        assert_ne!(SecretString::from("x"), SecretString::from("xx"));
    }

    #[test]
    fn empty_detection() {
        assert!(SecretString::default().is_empty());
        assert!(!SecretString::from("a").is_empty());
    }

    #[test]
    fn serializes_as_plain_string() {
        let json = serde_json::to_string(&SecretString::from("pw")).unwrap();
        assert_eq!(json, "\"pw\"");
        let back: SecretString = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expose(), "pw");
    }

    #[test]
    fn zeroize_clears_content() {
        let mut s = SecretString::from("secret");
        s.zeroize();
        assert!(s.is_empty());
    }
}
