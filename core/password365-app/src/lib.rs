//! UI-agnostic application service shared by every Password365 frontend.
//!
//! Frontends (desktop app, CLI, later mobile via FFI) are thin adapters: they
//! translate user input into calls on [`AppService`] and render the returned
//! DTOs. All business rules live here or below, so they are tested once.
//!
//! Security rule for frontends: item DTOs never contain secrets. A secret is
//! only returned by [`AppService::reveal`], one field at a time, on demand.

pub mod attachments;
pub mod attempts;
pub mod auto_lock;
pub mod auto_type;
pub mod backup;
pub mod breach;
pub mod browser;
pub mod cxf_export;
pub mod devices;
pub mod dto;
mod error;
pub mod field_layout;
pub mod item_icon;
pub mod item_input;
mod json_file;
pub mod onepux_import;
pub mod passkeys;
pub mod password_policy;
pub mod profile;
pub mod quick_unlock;
pub mod recently_deleted;
pub mod secret_key_store;
mod service;
pub mod settings;
pub mod ssh_agent;
pub mod transfer;
pub mod watchtower;
pub mod wifi;

pub use error::{AppError, Result};
pub use service::{AppConfig, AppService};
