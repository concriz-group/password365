//! The Emergency Kit PDF, kept in the vault as a Document item.

use password365_app::emergency_kit::KitText;
use password365_crypto::secret::SecretString;

use crate::{Password365, Result};

/// The sheet's words in the interface language.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EmergencyKitText {
    /// Title (also the item's title).
    pub title: String,
    /// "Created on …".
    pub created: String,
    /// Why to keep it safe.
    pub keep: String,
    /// "Secret Key".
    pub secret_key_label: String,
    /// "Master password".
    pub master_password_label: String,
    /// Hint beside the empty box.
    pub write_here: String,
    /// "How to use this sheet".
    pub how_to: String,
    /// The numbered steps.
    pub steps: Vec<String>,
}

#[uniffi::export]
impl Password365 {
    /// Keeps the Emergency Kit (a PDF with the Secret Key) in the vault, once, after checking
    /// the master password. Returns the item's id.
    pub fn save_emergency_kit(&self, password: String, text: EmergencyKitText) -> Result<String> {
        let text = KitText {
            title: text.title,
            created: text.created,
            keep: text.keep,
            secret_key_label: text.secret_key_label,
            master_password_label: text.master_password_label,
            write_here: text.write_here,
            how_to: text.how_to,
            steps: text.steps,
        };
        Ok(self
            .app()
            .save_emergency_kit(SecretString::from(password), &text)?)
    }

    /// An attached file's bytes (decrypted), to preview or share.
    pub fn attachment_bytes(&self, id: String, attachment: String) -> Result<Vec<u8>> {
        Ok(self.app().attachment_bytes(&id, &attachment)?.to_vec())
    }
}
