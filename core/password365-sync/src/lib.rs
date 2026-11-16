//! Serverless device-to-device sync for Password365.
//!
//! Items replicate as state-based CRDTs: every field is a last-writer-wins
//! register stamped with a Hybrid Logical Clock, password histories merge as
//! sets, and deletions remember what they had seen so an unseen edit wins.
//! See `docs/sync-protocol.md`.

mod device;
pub mod fields;
pub mod history;
mod hlc;
mod item_state;
pub mod legacy;
mod lww;
pub mod lww_map;
pub mod pairing;
mod sync_item;

pub use device::DeviceId;
pub use hlc::{Clock, Hlc, MAX_DRIFT_MS};
pub use item_state::{ItemState, StateMerge, SyncMeta};
pub use lww::Lww;
pub use lww_map::LwwMap;
pub use sync_item::{Merged, SyncItem, Tombstone};
