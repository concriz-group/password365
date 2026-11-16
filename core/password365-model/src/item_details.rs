//! Item details: the secret part of an item, decrypted only on demand.

use password365_crypto::secret::SecretString;
use serde::{Deserialize, Serialize};

use crate::{Attachment, PasswordHistory, Timestamp};

/// A user-defined extra field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomField {
    /// Field label, e.g. "Recovery code".
    pub label: String,
    /// Field value.
    pub value: SecretString,
    /// Hide the value in the UI until revealed.
    pub concealed: bool,
}

/// Secret fields of an item.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemDetails {
    /// Current password.
    pub password: Option<SecretString>,
    /// Free-form notes.
    pub notes: Option<SecretString>,
    /// TOTP secret or `otpauth://` URI.
    pub totp: Option<SecretString>,
    /// Extra fields.
    pub custom_fields: Vec<CustomField>,
    /// Previously used passwords.
    pub password_history: PasswordHistory,
    /// Files attached to the item (their bytes are stored apart).
    pub attachments: Vec<Attachment>,
}

/// [`ItemDetails`] as stored before attachments existed (format v1 and v2
/// records): read only, upgraded with no attachments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemDetailsV1 {
    /// Current password.
    pub password: Option<SecretString>,
    /// Free-form notes.
    pub notes: Option<SecretString>,
    /// TOTP secret or `otpauth://` URI.
    pub totp: Option<SecretString>,
    /// Extra fields.
    pub custom_fields: Vec<CustomField>,
    /// Previously used passwords.
    pub password_history: PasswordHistory,
}

impl From<ItemDetailsV1> for ItemDetails {
    fn from(old: ItemDetailsV1) -> Self {
        Self {
            password: old.password,
            notes: old.notes,
            totp: old.totp,
            custom_fields: old.custom_fields,
            password_history: old.password_history,
            attachments: Vec::new(),
        }
    }
}

/// Drops the attachments (tests and tools only: never stored).
impl From<&ItemDetails> for ItemDetailsV1 {
    fn from(current: &ItemDetails) -> Self {
        Self {
            password: current.password.clone(),
            notes: current.notes.clone(),
            totp: current.totp.clone(),
            custom_fields: current.custom_fields.clone(),
            password_history: current.password_history.clone(),
        }
    }
}

impl ItemDetails {
    /// Details holding only a password.
    #[must_use]
    pub fn with_password(password: SecretString) -> Self {
        Self {
            password: non_empty(password),
            ..Self::default()
        }
    }

    /// Sets notes (empty → none).
    #[must_use]
    pub fn notes(mut self, notes: SecretString) -> Self {
        self.notes = non_empty(notes);
        self
    }

    /// Replaces the password. The previous one is kept in the history.
    ///
    /// Returns `true` if the password actually changed.
    pub fn change_password(&mut self, new: Option<SecretString>, now: Timestamp) -> bool {
        let new = new.and_then(non_empty);
        if new == self.password {
            return false;
        }
        if let Some(old) = self.password.take() {
            self.password_history.record(old, now);
        }
        self.password = new;
        true
    }
}

fn non_empty(value: SecretString) -> Option<SecretString> {
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_password_ignores_empty() {
        assert_eq!(ItemDetails::with_password("".into()).password, None);
        assert_eq!(
            ItemDetails::with_password("x".into()).password,
            Some("x".into())
        );
    }

    #[test]
    fn notes_builder() {
        let d = ItemDetails::default().notes("hello".into());
        assert_eq!(d.notes, Some("hello".into()));
        assert_eq!(ItemDetails::default().notes("".into()).notes, None);
    }

    #[test]
    fn change_password_records_history() {
        let mut d = ItemDetails::with_password("one".into());
        assert!(d.change_password(Some("two".into()), Timestamp::from_millis(9)));
        assert_eq!(d.password, Some("two".into()));
        assert_eq!(d.password_history.entries()[0].password.expose(), "one");
        assert_eq!(
            d.password_history.entries()[0].replaced_at,
            Timestamp::from_millis(9)
        );
    }

    #[test]
    fn unchanged_password_is_a_no_op() {
        let mut d = ItemDetails::with_password("same".into());
        assert!(!d.change_password(Some("same".into()), Timestamp::now()));
        assert!(d.password_history.is_empty());
    }

    #[test]
    fn setting_first_password_has_no_history() {
        let mut d = ItemDetails::default();
        assert!(d.change_password(Some("new".into()), Timestamp::now()));
        assert!(d.password_history.is_empty());
    }

    #[test]
    fn clearing_password_keeps_it_in_history() {
        let mut d = ItemDetails::with_password("gone".into());
        assert!(d.change_password(Some("".into()), Timestamp::now()));
        assert_eq!(d.password, None);
        assert_eq!(d.password_history.len(), 1);
    }

    #[test]
    fn custom_field_debug_is_redacted() {
        let f = CustomField {
            label: "pin".into(),
            value: "1234".into(),
            concealed: true,
        };
        assert!(!format!("{f:?}").contains("1234"));
    }
}
