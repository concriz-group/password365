//! Files attached to items: attaching from disk, saving a copy, removing.
//! The bytes never cross into the window: the app reads and writes files
//! itself.

use std::path::Path;

use password365_model::attachment::MAX_ATTACHMENT_BYTES;
use password365_model::{Attachment, AttachmentId, ItemId};
use serde::Serialize;
use zeroize::Zeroizing;

use crate::service::parse_id;
use crate::{AppError, AppService, Result};

/// An attachment as the item view shows it (no key, no digest).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttachmentDto {
    /// Id.
    pub id: String,
    /// File name.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Media type ("" when unknown).
    pub content_type: String,
    /// The bytes are on this device (else they arrive with the next sync).
    pub available: bool,
}

impl AttachmentDto {
    /// The view of `a`.
    #[must_use]
    pub fn new(a: &Attachment, available: bool) -> Self {
        Self {
            id: a.id.to_string(),
            name: a.name.clone(),
            size: a.size,
            content_type: a.content_type.clone(),
            available,
        }
    }
}

/// A media type from the file name, for the common kinds of document.
#[must_use]
pub fn content_type_of(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pem" | "key" => "application/x-pem-file",
        _ => "",
    }
}

impl AppService {
    /// Attaches the file at `path` to item `id`.
    pub fn attach_file(&mut self, id: &str, path: &Path) -> Result<AttachmentDto> {
        let item = parse_id::<ItemId>(id)?;
        let size = std::fs::metadata(path)
            .map_err(|e| AppError::Io(format!("cannot read {}: {e}", path.display())))?
            .len();
        if size > MAX_ATTACHMENT_BYTES {
            return Err(AppError::Validation(format!(
                "files can be at most {} MB",
                MAX_ATTACHMENT_BYTES / (1024 * 1024)
            )));
        }
        let bytes = Zeroizing::new(
            std::fs::read(path)
                .map_err(|e| AppError::Io(format!("cannot read {}: {e}", path.display())))?,
        );
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_owned();
        let attachment = self
            .session()?
            .attach(item, &name, content_type_of(&name), &bytes)?;
        Ok(AttachmentDto::new(&attachment, true))
    }

    /// Writes a decrypted copy of an attachment to `dest` (readable only by
    /// this user where the system allows it).
    pub fn save_attachment(&mut self, id: &str, attachment: &str, dest: &Path) -> Result<()> {
        use std::io::Write as _;
        let bytes = self.attachment_bytes(id, attachment)?;
        let fail =
            |e: std::io::Error| AppError::Io(format!("cannot write {}: {e}", dest.display()));
        // The save dialog already asked before replacing an existing file.
        if dest.exists() {
            std::fs::remove_file(dest).map_err(fail)?;
        }
        password365_vault::private_file::create_private_file(dest)
            .and_then(|mut f| f.write_all(&bytes))
            .map_err(fail)
    }

    /// An attachment's decrypted bytes (the desktop app opens a temporary copy).
    pub fn attachment_bytes(&mut self, id: &str, attachment: &str) -> Result<Zeroizing<Vec<u8>>> {
        let item = parse_id::<ItemId>(id)?;
        let attachment = parse_id::<AttachmentId>(attachment)?;
        Ok(self.session()?.attachment_bytes(item, attachment)?)
    }

    /// The file name of an attachment.
    pub fn attachment_name(&mut self, id: &str, attachment: &str) -> Result<String> {
        let item = self.load_item(id)?;
        let wanted = parse_id::<AttachmentId>(attachment)?;
        item.details
            .attachments
            .iter()
            .find(|a| a.id == wanted)
            .map(|a| a.name.clone())
            .ok_or(AppError::NotFound("attachment"))
    }

    /// Removes an attachment from item `id`.
    pub fn remove_attachment(&mut self, id: &str, attachment: &str) -> Result<()> {
        let item = parse_id::<ItemId>(id)?;
        let attachment = parse_id::<AttachmentId>(attachment)?;
        Ok(self.session()?.detach(item, attachment)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item_input::ItemInput;
    use crate::service::tests::with_account;

    #[test]
    fn attaches_saves_and_removes_files() {
        let mut fx = with_account();
        let doc = fx
            .app
            .create_item(&ItemInput {
                kind: Some("document".into()),
                title: "Passport scan".into(),
                ..ItemInput::default()
            })
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("Passport.PDF");
        std::fs::write(&source, b"%PDF data").unwrap();
        let file = fx.app.attach_file(&doc.summary.id, &source).unwrap();
        assert_eq!(
            (
                file.name.as_str(),
                file.size,
                file.content_type.as_str(),
                file.available
            ),
            ("Passport.PDF", 9, "application/pdf", true)
        );
        let shown = fx.app.item(&doc.summary.id).unwrap();
        assert_eq!(shown.attachments, vec![file.clone()]);
        assert_eq!(
            fx.app.attachment_name(&doc.summary.id, &file.id).unwrap(),
            "Passport.PDF"
        );

        let copy = dir.path().join("copy.pdf");
        fx.app
            .save_attachment(&doc.summary.id, &file.id, &copy)
            .unwrap();
        assert_eq!(std::fs::read(&copy).unwrap(), b"%PDF data");

        fx.app.remove_attachment(&doc.summary.id, &file.id).unwrap();
        assert!(fx.app.item(&doc.summary.id).unwrap().attachments.is_empty());
        assert!(
            fx.app
                .attach_file(&doc.summary.id, &dir.path().join("missing"))
                .is_err()
        );
        assert!(fx.app.remove_attachment(&doc.summary.id, "nope").is_err());
        assert_eq!(content_type_of("a.unknown"), "");
    }
}
