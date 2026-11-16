//! Messages two paired devices exchange inside their encrypted channel.

use password365_sync::legacy::{ItemStateV1, ItemStateV2};
use password365_sync::{DeviceId, ItemState};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{Result, VaultError, codec};

/// Postcard framing overhead allowance per `Items` message (variant tag, length).
const ITEMS_OVERHEAD: usize = 16;

/// One sync message. New variants must be appended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    /// Opens a sync: who I am and how far I got with your changes.
    Hello {
        /// Sender's device id (must match the pinned key).
        device_id: DeviceId,
        /// Highest of *your* change sequences I already merged.
        have_from_you: Option<u64>,
    },
    /// A batch of item states from a format v1 device (read only: upgraded
    /// on arrival; never sent any more).
    ItemsV1(Vec<ItemStateV1>),
    /// "I have sent every change up to my sequence `seq`."
    Done {
        /// Sender's change sequence covered by the items sent so far.
        seq: u64,
    },
    /// "I merged everything you sent up to your sequence `seq`."
    Ack {
        /// The `Done.seq` being acknowledged.
        seq: u64,
    },
    /// Devices removed from the account (sent right after `Hello`).
    Revoked(Vec<DeviceId>),
    /// The account's latest master password change (after `Revoked`).
    Password(crate::PasswordChange),
    /// A batch of item states from a format v2 device (read only: upgraded
    /// on arrival; never sent any more).
    ItemsV2(Vec<ItemStateV2>),
    /// A batch of item states (format v3: per-field custom fields,
    /// attachments). An older device cannot read it and stops the sync, so
    /// nothing is merged half-understood.
    Items(Vec<ItemState>),
    /// "Send me these attachments' bytes" (named by items I now have).
    WantFiles(Vec<password365_model::AttachmentId>),
    /// A piece of an attachment's encrypted bytes, in order; `last` ends it.
    FilePart {
        /// The attachment.
        id: password365_model::AttachmentId,
        /// `true` on the final piece.
        last: bool,
        /// The bytes.
        bytes: Vec<u8>,
    },
    /// "I do not have this attachment's bytes either."
    NoFile(password365_model::AttachmentId),
    /// A piece of one item state too large for an `Items` message (its
    /// encoding, in order; `last` ends it). Older devices stop the sync.
    ItemPart {
        /// `true` on the final piece.
        last: bool,
        /// The bytes.
        bytes: Vec<u8>,
    },
}

/// Largest item state accepted in pieces (encoded).
pub const MAX_ITEM_BYTES: usize = 8 * 1024 * 1024;

/// Puts an item state sent as `ItemPart`s back together.
#[derive(Debug, Default)]
pub struct ItemPieces(Vec<u8>);

impl ItemPieces {
    /// Adds a piece; returns the item state once the last piece arrived.
    pub fn add(&mut self, last: bool, bytes: &[u8]) -> Result<Option<ItemState>> {
        if self.0.len() + bytes.len() > MAX_ITEM_BYTES {
            self.0.clear();
            return Err(VaultError::SyncProtocol("item too large"));
        }
        self.0.extend_from_slice(bytes);
        if !last {
            return Ok(None);
        }
        let whole = Zeroizing::new(std::mem::take(&mut self.0));
        codec::decode(&whole)
            .map(Some)
            .map_err(|_| VaultError::SyncProtocol("unreadable item"))
    }

    /// `true` while a piece is missing.
    #[must_use]
    pub fn waiting(&self) -> bool {
        !self.0.is_empty()
    }
}

/// Room left for the rest of a `FilePart` message (tag, id, flag, length).
const FILE_PART_OVERHEAD: usize = 64;

/// An attachment's encrypted bytes as `FilePart` messages that each fit `max_message`.
#[must_use]
pub fn file_parts(
    id: password365_model::AttachmentId,
    sealed: &[u8],
    max_message: usize,
) -> Vec<Message> {
    let size = max_message.saturating_sub(FILE_PART_OVERHEAD).max(1);
    let pieces: Vec<&[u8]> = if sealed.is_empty() {
        vec![&[]]
    } else {
        sealed.chunks(size).collect()
    };
    let count = pieces.len();
    pieces
        .into_iter()
        .enumerate()
        .map(|(i, bytes)| Message::FilePart {
            id,
            last: i + 1 == count,
            bytes: bytes.to_vec(),
        })
        .collect()
}

impl Message {
    /// Serializes for the channel (wiped from memory when dropped).
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        codec::encode(self)
    }

    /// Parses a message from the channel.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        codec::decode(bytes).map_err(|_| VaultError::SyncProtocol("unreadable message"))
    }

    /// The item states an `Items` message (of either format) carries, or
    /// the message itself when it carries none.
    pub fn into_states(self) -> std::result::Result<Vec<ItemState>, Self> {
        match self {
            Self::Items(states) => Ok(states),
            Self::ItemsV2(states) => Ok(states.into_iter().map(Into::into).collect()),
            Self::ItemsV1(states) => Ok(states.into_iter().map(Into::into).collect()),
            other => Err(other),
        }
    }
}

/// Messages on the pairing channel (new device joining an existing one).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairMessage {
    /// Existing device → new device: the join bundle (Secret Key, vaults, ...).
    Bundle(Vec<u8>),
    /// New device → existing device: its identity, once its store exists.
    Joined {
        /// The new device's sync id.
        device_id: DeviceId,
        /// Its name.
        name: String,
        /// Its long-term pairing key, to pin.
        static_public: [u8; 32],
    },
    /// Either side gave up (codes did not match, user cancelled).
    Cancelled,
}

impl PairMessage {
    /// Serializes for the channel (wiped from memory when dropped).
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        codec::encode(self)
    }

    /// Parses a message from the channel.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        codec::decode(bytes).map_err(|_| VaultError::SyncProtocol("unreadable pairing message"))
    }
}

/// Splits `states` into `Items` messages that each encode to at most `limit` bytes.
pub fn chunk(states: Vec<ItemState>, limit: usize) -> Result<Vec<Message>> {
    let budget = limit.saturating_sub(ITEMS_OVERHEAD);
    let mut out = Vec::new();
    let mut batch = Vec::new();
    let mut size = 0;
    for state in states {
        let encoded = codec::encode(&state)?;
        let len = encoded.len();
        if len > budget {
            // Too large for one message (long notes, big keys): in pieces.
            if !batch.is_empty() {
                out.push(Message::Items(std::mem::take(&mut batch)));
                size = 0;
            }
            if len > MAX_ITEM_BYTES {
                return Err(VaultError::SyncProtocol("item too large to send"));
            }
            let piece = limit.saturating_sub(FILE_PART_OVERHEAD).max(1);
            let count = encoded.len().div_ceil(piece);
            out.extend(
                encoded
                    .chunks(piece)
                    .enumerate()
                    .map(|(i, bytes)| Message::ItemPart {
                        last: i + 1 == count,
                        bytes: bytes.to_vec(),
                    }),
            );
            continue;
        }
        if size + len > budget {
            out.push(Message::Items(std::mem::take(&mut batch)));
            size = 0;
        }
        size += len;
        batch.push(state);
    }
    if !batch.is_empty() {
        out.push(Message::Items(batch));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use password365_model::{Item, ItemDetails, ItemKind, ItemOverview, Timestamp, VaultId};
    use password365_sync::{Clock, SyncItem};

    use super::*;

    fn state(title: &str) -> ItemState {
        let item = Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new(title).unwrap(),
            ItemDetails::with_password("pw".into()),
            Timestamp::from_millis(1),
        );
        ItemState::live(SyncItem::capture(
            &item,
            Clock::new(DeviceId::new()).tick(1),
        ))
    }

    #[test]
    fn messages_round_trip() {
        let all = [
            Message::Hello {
                device_id: DeviceId::new(),
                have_from_you: Some(3),
            },
            Message::Items(vec![state("a")]),
            Message::Done { seq: 9 },
            Message::Ack { seq: 9 },
        ];
        for message in all {
            assert_eq!(
                Message::decode(&message.encode().unwrap()).unwrap(),
                message
            );
        }
        assert!(matches!(
            Message::decode(b"\xff\xff"),
            Err(VaultError::SyncProtocol(_))
        ));
    }

    #[test]
    fn items_from_a_format_v1_device_are_upgraded() {
        let current = state("a");
        let old = Message::ItemsV1(vec![ItemStateV1::from(&current)]);
        let bytes = old.encode().unwrap();
        assert_eq!(bytes[0], 1, "same variant tag v1 devices used for Items");
        let states = Message::decode(&bytes).unwrap().into_states().unwrap();
        assert!(matches!(&states[..], [ItemState::Live(s)] if s.fields.title.value == "a"));
        assert!(Message::Done { seq: 1 }.into_states().is_err());
    }

    #[test]
    fn items_from_a_format_v2_device_are_upgraded() {
        let current = state("b");
        let old = Message::ItemsV2(vec![password365_sync::legacy::ItemStateV2::from(&current)]);
        let bytes = old.encode().unwrap();
        assert_eq!(bytes[0], 6, "same variant tag v2 devices used for Items");
        let states = Message::decode(&bytes).unwrap().into_states().unwrap();
        assert!(matches!(&states[..], [ItemState::Live(s)] if s.fields.title.value == "b"));
        assert_eq!(
            Message::Items(vec![]).encode().unwrap()[0],
            7,
            "v3 items have their own tag"
        );
    }

    #[test]
    fn files_are_split_into_parts_that_fit() {
        let id = password365_model::AttachmentId::new();
        let parts = file_parts(id, &[7u8; 300], 164);
        assert_eq!(parts.len(), 3);
        for part in &parts {
            assert!(part.encode().unwrap().len() <= 164);
        }
        assert!(matches!(
            parts.last(),
            Some(Message::FilePart { last: true, .. })
        ));
        assert!(
            matches!(&file_parts(id, &[], 164)[..], [Message::FilePart { last: true, bytes, .. }] if bytes.is_empty())
        );
    }

    #[test]
    fn chunks_respect_the_limit_and_keep_everything() {
        let states: Vec<ItemState> = (0..50).map(|i| state(&format!("item {i}"))).collect();
        let one = codec::encode(&states[0]).unwrap().len();
        let limit = one * 7 + ITEMS_OVERHEAD;
        let chunks = chunk(states.clone(), limit).unwrap();
        assert!(chunks.len() > 1);
        for c in &chunks {
            assert!(c.encode().unwrap().len() <= limit);
        }
        let rejoined: Vec<ItemState> = chunks
            .into_iter()
            .flat_map(|m| match m {
                Message::Items(s) => s,
                _ => vec![],
            })
            .collect();
        assert_eq!(rejoined, states);
        assert!(chunk(vec![], limit).unwrap().is_empty());
    }

    #[test]
    fn an_item_bigger_than_a_message_goes_in_pieces() {
        let messages = chunk(vec![state("a"), state("x"), state("b")], 120).unwrap();
        let mut pieces = ItemPieces::default();
        let mut states = Vec::new();
        for m in messages {
            match m {
                Message::Items(s) => states.extend(s),
                Message::ItemPart { last, bytes } => {
                    states.extend(pieces.add(last, &bytes).unwrap());
                }
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(states.len(), 3);
        assert!(!pieces.waiting());
    }
}
