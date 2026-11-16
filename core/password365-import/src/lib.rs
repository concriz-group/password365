//! Import from and export to browser password CSV files.
//!
//! - [`csv_import`]: reads Chrome / Edge / Brave / Opera CSV (and Firefox, Safari,
//!   Bitwarden-style columns) into [`ImportedLogin`]s.
//! - [`dedupe`]: compares imported logins with existing items.
//! - [`chrome_export`]: writes items in Chrome's CSV format.
//!
//! ⚠️ CSV files are **plaintext**. Callers must warn the user and offer to
//! delete the file afterwards.

pub mod bitwarden;
pub mod chrome_export;
mod columns;
pub mod csv_import;
pub mod cxf;
pub mod dedupe;
mod error;
mod imported;
pub mod item;
pub mod kdbx;
pub mod onepux;
pub mod url_key;

pub use error::{ImportError, Result};
pub use imported::ImportedLogin;
