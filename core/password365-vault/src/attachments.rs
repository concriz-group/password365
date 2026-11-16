//! Attaching files to items. Each file is encrypted with its own random key,
//! which lives only inside the encrypted item (the [`Attachment`] record).
//! The encrypted bytes are stored apart from the records, are never loaded
//! on unlock, and can be copied between devices as they are.

use std::collections::HashSet;

use password365_crypto::aad::Aad;
use password365_crypto::key::SymmetricKey;
use password365_crypto::{aead, random};
use password365_model::{Attachment, AttachmentId, ItemId, Timestamp};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::session::Session;
use crate::{Result, VaultError};

fn aad(id: AttachmentId) -> Aad {
    Aad::new("password365/v3/attachment").bytes(id.as_bytes())
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            use std::fmt::Write as _;
            let _ = write!(out, "{b:02x}");
            out
        })
}

fn key_of(attachment: &Attachment) -> Result<SymmetricKey> {
    let text = attachment.key.expose();
    let bytes: Zeroizing<Vec<u8>> = Zeroizing::new(
        (0..text.len())
            .step_by(2)
            .map(|i| {
                text.get(i..i + 2)
                    .and_then(|b| u8::from_str_radix(b, 16).ok())
            })
            .collect::<Option<Vec<u8>>>()
            .ok_or(VaultError::Corrupt("attachment key"))?,
    );
    SymmetricKey::from_slice(&bytes).map_err(|_| VaultError::Corrupt("attachment key"))
}

/// SHA-256 of `bytes`, hex.
#[must_use]
pub fn digest_of(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Encrypts a new file; returns its record (for the item) and its encrypted bytes.
pub fn encrypt(name: &str, content_type: &str, plaintext: &[u8]) -> Result<(Attachment, Vec<u8>)> {
    let id = AttachmentId::new();
    let raw: Zeroizing<[u8; 32]> = Zeroizing::new(random::bytes()?);
    let key = SymmetricKey::from_bytes(*raw);
    let sealed = aead::seal(&key, plaintext, &aad(id))?;
    let attachment = Attachment {
        id,
        name: name.trim().to_owned(),
        size: u64::try_from(plaintext.len()).unwrap_or(u64::MAX),
        content_type: content_type.to_owned(),
        key: hex(raw.as_ref()).into(),
        digest: digest_of(&sealed),
    };
    attachment.validate()?;
    Ok((attachment, sealed))
}

/// Decrypts stored bytes after checking they are the ones the item names.
pub fn decrypt(attachment: &Attachment, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if digest_of(sealed) != attachment.digest {
        return Err(VaultError::Corrupt("attachment bytes"));
    }
    aead::open(&key_of(attachment)?, sealed, &aad(attachment.id))
        .map_err(|_| VaultError::Corrupt("attachment bytes"))
}

impl Session {
    /// Attaches a file to item `item`; returns the new attachment.
    pub fn attach(
        &mut self,
        item: ItemId,
        name: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<Attachment> {
        let mut stored = self.item(item)?;
        let (attachment, sealed) = encrypt(name, content_type, bytes)?;
        self.db.put_attachment(attachment.id.as_bytes(), &sealed)?;
        stored.details.attachments.push(attachment.clone());
        stored.touch(Timestamp::now());
        if let Err(e) = self.save_item(&stored) {
            let _ = self.db.delete_attachment(attachment.id.as_bytes());
            return Err(e);
        }
        Ok(attachment)
    }

    /// Removes an attachment from item `item` (its bytes go once nothing refers to them).
    pub fn detach(&mut self, item: ItemId, attachment: AttachmentId) -> Result<()> {
        let mut stored = self.item(item)?;
        let before = stored.details.attachments.len();
        stored.details.attachments.retain(|a| a.id != attachment);
        if stored.details.attachments.len() == before {
            return Err(VaultError::NotFound("attachment"));
        }
        stored.touch(Timestamp::now());
        self.save_item(&stored)
    }

    /// An attachment's decrypted bytes.
    pub fn attachment_bytes(
        &self,
        item: ItemId,
        attachment: AttachmentId,
    ) -> Result<Zeroizing<Vec<u8>>> {
        let stored = self.item(item)?;
        let found = stored
            .details
            .attachments
            .iter()
            .find(|a| a.id == attachment)
            .ok_or(VaultError::NotFound("attachment"))?;
        let sealed = self
            .db
            .attachment(found.id.as_bytes())?
            .ok_or(VaultError::NotFound("attachment bytes"))?;
        decrypt(found, &sealed)
    }

    /// Every attachment the items (Recently Deleted included) refer to.
    pub fn referenced_attachments(&self) -> Result<Vec<Attachment>> {
        let mut all = Vec::new();
        for entry in self.items.values() {
            let key = &self.vault_entry(entry.record.vault_id)?.key;
            all.extend(
                crate::item_crypto::open_item(key, &entry.record)?
                    .details
                    .attachments,
            );
        }
        Ok(all)
    }

    /// Attachments the items refer to whose bytes this device does not have
    /// yet (they come with the next sync).
    pub fn missing_attachments(&self) -> Result<Vec<Attachment>> {
        let stored: HashSet<[u8; 16]> = self.db.attachment_ids()?.into_iter().collect();
        Ok(self
            .referenced_attachments()?
            .into_iter()
            .filter(|a| !stored.contains(a.id.as_bytes()))
            .collect())
    }

    /// Whether this device has the bytes of attachment `id`.
    pub fn has_attachment(&self, id: AttachmentId) -> Result<bool> {
        self.db.has_attachment(id.as_bytes())
    }

    /// Encrypted bytes of an attachment, to send to a paired device.
    pub fn sealed_attachment(&self, id: AttachmentId) -> Result<Option<Vec<u8>>> {
        self.db.attachment(id.as_bytes())
    }

    /// Stores encrypted bytes received from a paired device, if an item here
    /// names them (and they match its digest). Returns whether they were kept.
    pub fn store_received_attachment(&mut self, id: AttachmentId, sealed: &[u8]) -> Result<bool> {
        let wanted = self
            .referenced_attachments()?
            .into_iter()
            .find(|a| a.id == id);
        match wanted {
            Some(a) if a.digest == digest_of(sealed) => {
                self.db.put_attachment(id.as_bytes(), sealed)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Deletes stored bytes no item refers to any more; returns how many.
    pub fn collect_unreferenced_attachments(&mut self) -> Result<usize> {
        let referenced: HashSet<[u8; 16]> = self
            .referenced_attachments()?
            .iter()
            .map(|a| *a.id.as_bytes())
            .collect();
        let mut removed = 0;
        for id in self.db.attachment_ids()? {
            if !referenced.contains(&id) {
                self.db.delete_attachment(&id)?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview};

    use super::*;
    use crate::store::test_support::new_session;

    fn note(s: &mut Session) -> ItemId {
        let vault = s.vault_by_name("Personal").unwrap().id;
        s.add_item(
            vault,
            ItemKind::SecureNote,
            ItemOverview::new("Passport").unwrap(),
            ItemDetails::default(),
        )
        .unwrap()
    }

    #[test]
    fn files_are_attached_read_back_and_detached() {
        let (_dir, mut s) = new_session();
        let id = note(&mut s);
        let file = s
            .attach(id, "passport.pdf", "application/pdf", b"%PDF-1.7 hello")
            .unwrap();
        assert_eq!((file.size, file.name.as_str()), (14, "passport.pdf"));
        assert_eq!(s.item(id).unwrap().details.attachments, vec![file.clone()]);
        assert_eq!(
            s.attachment_bytes(id, file.id).unwrap().as_slice(),
            b"%PDF-1.7 hello"
        );
        assert!(s.missing_attachments().unwrap().is_empty());
        let sealed = s.sealed_attachment(file.id).unwrap().unwrap();
        assert!(
            !sealed.windows(5).any(|w| w == b"hello"),
            "stored encrypted"
        );

        s.detach(id, file.id).unwrap();
        assert!(s.item(id).unwrap().details.attachments.is_empty());
        assert!(s.detach(id, file.id).is_err());
        assert_eq!(s.collect_unreferenced_attachments().unwrap(), 1);
        assert!(s.sealed_attachment(file.id).unwrap().is_none());
    }

    #[test]
    fn deleted_items_keep_their_files_until_purged() {
        let (_dir, mut s) = new_session();
        let id = note(&mut s);
        let file = s.attach(id, "scan.png", "image/png", &[1, 2, 3]).unwrap();
        s.delete_item(id).unwrap();
        assert_eq!(
            s.collect_unreferenced_attachments().unwrap(),
            0,
            "Recently Deleted can still restore it"
        );
        s.purge_deleted(Timestamp::from_millis(i64::MAX)).unwrap();
        assert_eq!(s.collect_unreferenced_attachments().unwrap(), 1);
        assert!(s.sealed_attachment(file.id).unwrap().is_none());
    }

    #[test]
    fn received_bytes_are_kept_only_if_an_item_names_them_unchanged() {
        let (_dir, mut s) = new_session();
        let id = note(&mut s);
        let file = s.attach(id, "a.txt", "text/plain", b"secret").unwrap();
        let sealed = s.sealed_attachment(file.id).unwrap().unwrap();
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(!s.store_received_attachment(file.id, &tampered).unwrap());
        assert!(
            !s.store_received_attachment(AttachmentId::new(), &sealed)
                .unwrap()
        );
        assert!(s.store_received_attachment(file.id, &sealed).unwrap());
        assert!(decrypt(&file, &tampered).is_err());
    }

    #[test]
    fn too_big_or_badly_named_files_are_refused() {
        let (_dir, mut s) = new_session();
        let id = note(&mut s);
        assert!(s.attach(id, "../evil", "", b"x").is_err());
        assert!(s.attach(id, "  ", "", b"x").is_err());
        assert!(s.item(id).unwrap().details.attachments.is_empty());
        assert_eq!(
            s.db.attachment_ids().unwrap().len(),
            0,
            "nothing stored for refused files"
        );
    }
}
