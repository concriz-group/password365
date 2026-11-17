//! `.p365sync` files: sync without a network (USB stick, nearby sharing, e-mail to
//! yourself).
//!
//! A file is written for **one** paired device. It is encrypted to that
//! device's pinned key and authenticated by the sender's key (Noise `X`), so
//! nobody else can read it, and nobody can forge or alter one.
//!
//! Layout: `QLSYNC1\n` followed by frames (2-byte length + body):
//! 1. the Noise handshake message;
//! 2. sealed `Hello { device_id, have_from_you }`, then `Revoked(ids)` and
//!    the latest `Password` change if any;
//! 3. sealed `Items` batches;
//! 4. sealed `Done { seq }`;
//! 5. sealed `FilePart`s: the encrypted bytes of the attachments those items
//!    name (the sender's copy), kept only if they match the item's digest.
//!
//! Nothing is applied unless the whole file is read and authentic.
//! Importing the same file twice changes nothing (merges are idempotent).

use std::io::Cursor;

use password365_model::Timestamp;
use password365_sync::DeviceId;
use password365_sync::ItemState;
use password365_sync::pairing::Handshake;
use password365_sync::pairing::noise::MAX_PLAINTEXT;
use password365_vault::wire::{Message, chunk, file_parts};
use password365_vault::{Session, VaultError};

use crate::sync::SyncOutcome;
use crate::{Result, frame};

/// File signature.
pub const MAGIC: &[u8; 8] = b"QLSYNC1\n";
/// File extension.
pub const EXTENSION: &str = "p365sync";

/// Writes the changes `peer` is missing into a file only it can read.
pub fn export(session: &Session, peer: DeviceId) -> Result<Vec<u8>> {
    let target = session.peer(peer)?;
    let mut handshake = Handshake::file_sender(&session.static_key(), &target.static_public)?;
    let first = handshake.write()?;
    let mut channel = handshake.finish()?;
    let mut messages = vec![
        Message::Hello {
            device_id: session.device_id(),
            have_from_you: target.received_seq,
        },
        Message::Revoked(session.revoked_devices()),
    ];
    if let Some(change) = session.password_change() {
        messages.push(Message::Password(change.clone()));
    }
    let changes = session.changes_for(target, target.acked_seq)?;
    let attachments: Vec<_> = changes
        .iter()
        .filter_map(|state| match state {
            ItemState::Live(item) => Some(item.fields.attachments.values()),
            ItemState::Purged { .. } => None,
        })
        .flatten()
        .collect();
    messages.extend(chunk(changes, MAX_PLAINTEXT)?);
    messages.push(Message::Done {
        seq: session.sync_seq(),
    });
    for attachment in attachments {
        if let Some(sealed) = session.sealed_attachment(attachment.id)? {
            messages.extend(file_parts(attachment.id, &sealed, MAX_PLAINTEXT));
        }
    }

    let mut out = MAGIC.to_vec();
    frame::write(&mut out, &first)?;
    for message in messages {
        frame::write(&mut out, &channel.seal(&message.encode()?)?)?;
    }
    Ok(out)
}

/// Reads a file from a paired device and merges it.
pub fn import(session: &mut Session, bytes: &[u8]) -> Result<SyncOutcome> {
    let body = bytes
        .strip_prefix(MAGIC.as_slice())
        .ok_or(VaultError::NotASyncFile)?;
    let mut input = Cursor::new(body);
    let mut handshake = Handshake::file_receiver(&session.static_key())?;
    handshake.read(&frame::read(&mut input)?)?;
    let mut channel = handshake.finish()?;
    let sender = session
        .peers()
        .into_iter()
        .find(|p| p.static_public == channel.remote_static())
        .map(|p| p.device_id)
        .ok_or(VaultError::UnknownDevice)?;

    let total = body.len();
    // `None` once the whole file has been read.
    let mut next = || -> Result<Option<Message>> {
        if usize::try_from(input.position()).unwrap_or(usize::MAX) >= total {
            return Ok(None);
        }
        let sealed = frame::read(&mut input).map_err(|_| VaultError::NotASyncFile)?;
        Ok(Some(Message::decode(&channel.open(&sealed)?)?))
    };
    let mut expect =
        || -> Result<Message> { next()?.ok_or_else(|| VaultError::NotASyncFile.into()) };
    let Message::Hello {
        device_id,
        have_from_you,
    } = expect()?
    else {
        return Err(VaultError::SyncProtocol("sync file does not start with hello").into());
    };
    if device_id != sender {
        return Err(VaultError::SyncProtocol("sync file sender mismatch").into());
    }
    let mut states = Vec::new();
    let mut pieces = password365_vault::wire::ItemPieces::default();
    let mut revoked = Vec::new();
    let mut password = None;
    let done = loop {
        match expect()? {
            items @ (Message::Items(_) | Message::ItemsV2(_) | Message::ItemsV1(_)) => {
                states.extend(items.into_states().unwrap_or_default());
            }
            Message::ItemPart { last, bytes } => states.extend(pieces.add(last, &bytes)?),
            Message::Revoked(ids) if ids.contains(&sender) => {
                return Err(VaultError::RevokedDevice.into());
            }
            Message::Revoked(ids) => revoked.extend(ids),
            Message::Password(change) => password = Some(change),
            Message::Done { seq } => break seq,
            _ => return Err(VaultError::SyncProtocol("unexpected message in sync file").into()),
        }
    };
    let mut files: Vec<(password365_model::AttachmentId, Vec<u8>)> = Vec::new();
    let mut part: Option<(password365_model::AttachmentId, Vec<u8>)> = None;
    while let Some(message) = next()? {
        match message {
            Message::FilePart { id, last, bytes } => {
                let (_, buffer) = part.get_or_insert_with(|| (id, Vec::new()));
                buffer.extend_from_slice(&bytes);
                if last {
                    files.extend(part.take());
                }
            }
            _ => return Err(VaultError::SyncProtocol("unexpected message in sync file").into()),
        }
    }
    let from = session.peer(sender)?.clone();
    let mut report = session.apply_changes_from(&from, &states)?;
    for (id, sealed) in files {
        if session.store_received_attachment(id, &sealed)? {
            report.files += 1;
        }
    }
    session.merge_revoked_from(sender, &revoked)?;
    if let Some(change) = password {
        report.password_changed = session.merge_password_change(change)?;
    }
    session.record_sync(sender, have_from_you, Some(done), Timestamp::now())?;
    Ok(SyncOutcome {
        peer: sender,
        report,
    })
}
