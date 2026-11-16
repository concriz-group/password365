//! A file attached to an item (1Password's Document category and "Attach a
//! File"). The item only holds this description; the encrypted bytes live
//! apart in the store and travel separately when devices sync.

use password365_crypto::secret::SecretString;
use serde::{Deserialize, Serialize};

use crate::ids::AttachmentId;

/// Largest file that can be attached (bytes).
pub const MAX_ATTACHMENT_BYTES: u64 = 50 * 1024 * 1024;

/// A file attached to an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    /// Identifier (also of the stored bytes).
    pub id: AttachmentId,
    /// File name as the user knows it.
    pub name: String,
    /// Size of the file (plaintext), bytes.
    pub size: u64,
    /// Media type (`application/pdf` …); empty when unknown.
    pub content_type: String,
    /// The file's own 256-bit key (hex), only ever stored inside the
    /// encrypted item: the bytes can then travel and be stored as they are.
    pub key: SecretString,
    /// SHA-256 of the encrypted bytes (hex): checks a received copy.
    pub digest: String,
}

impl Attachment {
    /// Checks the name and size.
    pub fn validate(&self) -> crate::Result<()> {
        let name = self.name.trim();
        if name.is_empty() || name.len() > 255 || name.contains(['/', '\\', '\0']) {
            return Err(crate::ModelError::Invalid(
                "a file name must not be empty or contain / \\".into(),
            ));
        }
        if self.size > MAX_ATTACHMENT_BYTES {
            return Err(crate::ModelError::Invalid(format!(
                "files can be at most {} MB",
                MAX_ATTACHMENT_BYTES / (1024 * 1024)
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: u64) -> Attachment {
        Attachment {
            id: AttachmentId::new(),
            name: name.into(),
            size,
            content_type: String::new(),
            key: "00".into(),
            digest: String::new(),
        }
    }

    #[test]
    fn names_and_sizes_are_checked() {
        assert!(file("passport.pdf", 10).validate().is_ok());
        for bad in ["", "  ", "a/b", "..\\x"] {
            assert!(file(bad, 1).validate().is_err(), "{bad}");
        }
        assert!(
            file("big.iso", MAX_ATTACHMENT_BYTES + 1)
                .validate()
                .is_err()
        );
    }
}
