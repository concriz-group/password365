//! Plain data model for Password365: identifiers, vaults, items and their fields.
//!
//! This crate contains no I/O and no cryptography — only data and the
//! business rules that apply to it (validation, password history …).

pub mod attachment;
mod error;
pub mod ids;
pub mod item;
pub mod item_details;
pub mod item_kind;
pub mod item_overview;
pub mod item_template;
pub mod password_history;
pub mod timestamp;
pub mod vault;

pub use attachment::Attachment;
pub use error::{ModelError, Result};
pub use ids::{AttachmentId, ItemId, VaultId};
pub use item::Item;
pub use item_details::{CustomField, ItemDetails, ItemDetailsV1};
pub use item_kind::ItemKind;
pub use item_overview::ItemOverview;
pub use password_history::{PasswordHistory, PasswordHistoryEntry};
pub use timestamp::Timestamp;
pub use vault::Vault;
