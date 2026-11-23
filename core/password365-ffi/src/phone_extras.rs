//! What the desktop app has and a phone needs too: Recently Deleted, export, attaching files, the Wi-Fi
//! code, shared by the iPhone and Android apps.

use std::path::Path;

use password365_crypto::secret::SecretString;

use crate::{AttachmentRow, Password365, Result};

/// An item in Recently Deleted.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DeletedItemView {
    /// Item id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Username, if any.
    pub username: Option<String>,
    /// Category.
    pub kind: String,
    /// When it was deleted (ms since epoch).
    pub deleted_at: i64,
    /// When it will be deleted for good (ms since epoch).
    pub purge_at: i64,
}

#[uniffi::export]
impl Password365 {
    /// Items in Recently Deleted, most recently deleted first (they go away by themselves after 30 days).
    pub fn deleted_items(&self) -> Result<Vec<DeletedItemView>> {
        Ok(self
            .app()
            .deleted_items()?
            .into_iter()
            .map(|d| DeletedItemView {
                id: d.summary.id,
                title: d.summary.title,
                username: d.summary.username,
                kind: d.summary.kind,
                deleted_at: d.deleted_at,
                purge_at: d.purge_at,
            })
            .collect())
    }

    /// Brings an item back from Recently Deleted.
    pub fn restore_item(&self, id: String) -> Result<()> {
        Ok(self.app().restore_item(&id)?)
    }

    /// Deletes an item from Recently Deleted for good.
    pub fn delete_permanently(&self, id: String) -> Result<()> {
        Ok(self.app().delete_permanently(&id)?)
    }

    /// Exports every login to a new owner-only Chrome-style CSV file (PLAINTEXT), after the master
    /// password is typed again. Returns how many were written.
    pub fn export_csv(&self, path: String, password: String) -> Result<u64> {
        Ok(self
            .app()
            .export_csv(Path::new(&path), None, SecretString::from(password))? as u64)
    }

    /// Exports every item to a new owner-only Credential Exchange (CXF) file (PLAINTEXT), after the
    /// master password is typed again. Returns how many were written.
    pub fn export_cxf(&self, path: String, password: String) -> Result<u64> {
        Ok(self
            .app()
            .export_cxf(Path::new(&path), None, SecretString::from(password))? as u64)
    }

    /// Attaches the file at `path` to item `id` (it is encrypted into the vault; the caller deletes its copy).
    pub fn attach_file(&self, id: String, path: String) -> Result<AttachmentRow> {
        let a = self.app().attach_file(&id, Path::new(&path))?;
        Ok(AttachmentRow {
            id: a.id,
            name: a.name,
            size: a.size,
            content_type: a.content_type,
            available: a.available,
        })
    }

    /// Removes an attached file.
    pub fn remove_attachment(&self, id: String, attachment: String) -> Result<()> {
        Ok(self.app().remove_attachment(&id, &attachment)?)
    }

    /// The `WIFI:` text of a Wireless Router item (a QR code of it joins the network), or none.
    pub fn wifi_join_text(&self, id: String) -> Result<Option<String>> {
        Ok(self.app().wifi_join_text(&id)?)
    }
}
