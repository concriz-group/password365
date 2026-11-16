//! Thread-safe holder for the single `AppService` instance.

use std::sync::{Arc, Mutex};

use password365_app::{AppError, AppService, Result};

/// Event emitted to the frontend whenever the vault is locked or unlocked
/// outside the window (idle auto-lock, unlock from the browser extension).
pub const STATUS_EVENT: &str = "vault-status-changed";

/// The state registered with Tauri (`app.manage`) and read by every command.
/// Commands must ask for exactly this type: Tauri looks state up by type.
pub type SharedState = Arc<AppState>;

/// Thread-safe wrapper around the service.
pub struct AppState(Mutex<AppService>);

impl AppState {
    /// Wraps a service.
    #[must_use]
    pub fn new(service: AppService) -> Self {
        Self(Mutex::new(service))
    }

    /// Runs `f` with exclusive access to the service.
    pub fn with<T>(&self, f: impl FnOnce(&mut AppService) -> Result<T>) -> Result<T> {
        let mut guard = self
            .0
            .lock()
            .map_err(|_| AppError::Internal("application state poisoned".into()))?;
        f(&mut guard)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use password365_app::AppConfig;
    use password365_app::secret_key_store::MemorySecretKeyStore;
    use password365_crypto::kdf::KdfParams;

    use super::AppState;

    /// A fresh state backed by a temp dir (keep the `TempDir` alive).
    pub(crate) fn state() -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig {
            store_path: dir.path().join("store.qlvault"),
            kdf: KdfParams::insecure_for_tests(),
            auto_lock_ms: 0,
            settings_path: None,
        };
        let service =
            password365_app::AppService::new(config, Box::new(MemorySecretKeyStore::default()));
        (dir, AppState::new(service))
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::state;

    #[test]
    fn with_gives_access_to_service() {
        let (_d, state) = state();
        let status = state.with(|s| Ok(s.status())).unwrap();
        assert!(!status.initialized);
    }

    #[test]
    fn errors_propagate() {
        let (_d, state) = state();
        let err = state.with(password365_app::AppService::vaults).unwrap_err();
        assert_eq!(err.code(), "locked");
    }
}
