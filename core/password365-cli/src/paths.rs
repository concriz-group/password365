//! Default locations for the store and the Secret Key file.
//!
//! The Secret Key is kept in a different file from the store so that copying
//! the store alone (backup, sync bundle, stolen file) is not enough to attack it.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

/// Default store file name.
pub const STORE_FILE: &str = "default.qlvault";
/// Default Secret Key file name.
pub const SECRET_KEY_FILE: &str = "secret-key";

/// Resolved file locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Vault store.
    pub store: PathBuf,
    /// Secret Key file.
    pub secret_key: PathBuf,
}

impl Paths {
    /// Uses explicit paths when given, otherwise platform defaults.
    pub fn resolve(store: Option<PathBuf>, secret_key: Option<PathBuf>) -> Result<Self> {
        let dirs = || {
            ProjectDirs::from("app", "Password365", "Password365")
                .context("cannot determine home directory")
        };
        let store = match store {
            Some(p) => p,
            None => dirs()?.data_dir().join(STORE_FILE),
        };
        let secret_key = match secret_key {
            Some(p) => p,
            None => dirs()?.config_local_dir().join(SECRET_KEY_FILE),
        };
        Ok(Self { store, secret_key })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_paths_win() {
        let p = Paths::resolve(Some("/a/s".into()), Some("/a/k".into())).unwrap();
        assert_eq!(
            p,
            Paths {
                store: "/a/s".into(),
                secret_key: "/a/k".into()
            }
        );
    }

    #[test]
    fn defaults_use_standard_file_names() {
        let p = Paths::resolve(None, None).unwrap();
        assert!(p.store.ends_with(STORE_FILE));
        assert!(p.secret_key.ends_with(SECRET_KEY_FILE));
    }
}
