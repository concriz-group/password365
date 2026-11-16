//! OS credential store adapter (macOS Keychain, Windows Credential Manager,
//! Linux Secret Service) for the Secret Key.

use password365_app::secret_key_store::SecretSlot;
use password365_app::{AppError, Result};

/// Service name under which Password365 stores its credentials.
pub const SERVICE: &str = "app.password365.desktop";
/// Account name of the Secret Key entry.
pub const SECRET_KEY_ACCOUNT: &str = "secret-key";

/// One keychain entry.
pub struct KeychainSlot {
    entry: keyring::Entry,
}

impl KeychainSlot {
    /// Opens the Secret Key entry, or `None` if no OS credential store is usable
    /// (e.g. a Linux session without a Secret Service daemon).
    pub fn secret_key() -> Option<Self> {
        Self::open(SERVICE)
    }

    /// The entry the app used under its former name (Quillock), to move the
    /// Secret Key from. Not on macOS: the Keychain ties entries to the app's
    /// identity, so reading it would show a password dialog; the unlock
    /// screen asks for the Secret Key once instead.
    pub fn legacy_secret_key() -> Option<Self> {
        if cfg!(target_os = "macos") {
            return None;
        }
        Self::open(crate::legacy::LEGACY_IDENTIFIER)
    }

    fn open(service: &str) -> Option<Self> {
        keyring::Entry::store_status().as_ref().ok()?;
        keyring::Entry::new(service, SECRET_KEY_ACCOUNT)
            .ok()
            .map(|entry| Self { entry })
    }
}

impl SecretSlot for KeychainSlot {
    fn get(&self) -> Result<Option<String>> {
        match self.entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(to_app_error(&e)),
        }
    }

    fn set(&self, value: &str) -> Result<()> {
        self.entry.set_password(value).map_err(|e| to_app_error(&e))
    }

    fn delete(&self) -> Result<()> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(to_app_error(&e)),
        }
    }
}

fn to_app_error(e: &keyring::Error) -> AppError {
    AppError::Io(format!("system keychain: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_reported_as_io() {
        assert_eq!(to_app_error(&keyring::Error::NoDefaultStore).code(), "io");
    }

    /// Touches the real OS keychain under a throw-away account; run manually with
    /// `cargo test -- --ignored`.
    #[test]
    #[ignore = "uses the real OS keychain"]
    fn real_keychain_round_trip() {
        let slot = KeychainSlot {
            entry: keyring::Entry::new(SERVICE, "test-round-trip").unwrap(),
        };
        slot.delete().unwrap();
        assert_eq!(slot.get().unwrap(), None);
        slot.set("value").unwrap();
        assert_eq!(slot.get().unwrap().as_deref(), Some("value"));
        slot.delete().unwrap();
        assert_eq!(slot.get().unwrap(), None);
    }
}
