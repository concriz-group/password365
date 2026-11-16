//! Sync without a shared network: a `.p365sync` file carries the changes a
//! paired device is missing (shared by `AirDrop`, Files, Mail…). Only that
//! device can read it, and a file is applied only if it comes, complete and
//! authentic, from a paired device.

use std::path::Path;

use crate::{Password365, Result};

/// What importing a sync file did.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncFileImport {
    /// Name of the device that wrote it.
    pub device: String,
    /// Items created, changed or deleted here.
    pub changed: u64,
    /// Items with two different new passwords (to review).
    pub conflicts: u64,
}

#[uniffi::export]
impl Password365 {
    /// Writes a sync file for the paired device `device_id` at `path`;
    /// returns how many changes it carries.
    pub fn export_sync_file(&self, device_id: String, path: String) -> Result<u64> {
        Ok(self.app().export_sync_file(&device_id, Path::new(&path))? as u64)
    }

    /// Reads a sync file (from a paired device) and merges it.
    pub fn import_sync_file(&self, path: String) -> Result<SyncFileImport> {
        let result = self.app().import_sync_file(Path::new(&path))?;
        Ok(SyncFileImport {
            device: result.device.name,
            changed: result.changed as u64,
            conflicts: result.conflicts.len() as u64,
        })
    }
}
