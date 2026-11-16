//! Encrypted local storage for Password365.
//!
//! On disk (SQLite) there are only two tables:
//! - `header`  – store id, KDF parameters, salt and the wrapped account keyset;
//! - `records` – opaque `(id, blob)` pairs, each blob encrypted and size-padded.
//!
//! Nothing else — no titles, URLs, vault names or item types — is visible
//! without the master password and the Secret Key.
//!
//! Entry points: [`Store::create`], [`Store::open`] → [`Store::unlock`] → [`Session`].

pub mod attachments;
mod codec;
mod context;
mod db;
mod envelope;
mod error;
mod header;
mod item_crypto;
mod item_summary;
mod join;
mod padding;
pub mod peer_sync;
pub mod private_file;
mod quick_unlock;
mod records;
mod session;
mod session_items;
mod session_password;
mod session_peers;
mod session_review;
mod session_revoked;
mod session_sync;
mod session_vaults;
mod store;
mod sync_meta;
pub mod wire;

pub use error::{Result, VaultError};
pub use item_summary::ItemSummary;
pub use session::Session;
pub use session_password::PasswordChange;
pub use session_peers::{DEFAULT_DEVICE_NAME, MAX_DEVICE_NAME, Peer};
pub use session_sync::ApplyReport;
pub use store::{Credentials, DEFAULT_VAULT_NAME, Store};
