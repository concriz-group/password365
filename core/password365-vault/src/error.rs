use password365_crypto::CryptoError;
use password365_model::ModelError;
use thiserror::Error;

/// Errors from the vault layer.
#[derive(Debug, Error)]
pub enum VaultError {
    /// Master password or Secret Key is wrong (deliberately not saying which).
    #[error("wrong master password or Secret Key")]
    WrongCredentials,
    /// Underlying cryptographic failure.
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    /// Validation error from the model.
    #[error(transparent)]
    Model(#[from] ModelError),
    /// SQLite error.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// The store already exists at the given path.
    #[error("a vault store already exists at this location")]
    AlreadyExists,
    /// No store at the given path.
    #[error("no vault store found at this location")]
    NotInitialized,
    /// The store uses a format this build does not understand.
    #[error("unsupported store format version {0}")]
    UnsupportedFormat(u32),
    /// Stored data is structurally invalid.
    #[error("corrupt data: {0}")]
    Corrupt(&'static str),
    /// A vault or item does not exist.
    #[error("{0} not found")]
    NotFound(&'static str),
    /// A vault still contains items.
    #[error("vault is not empty")]
    VaultNotEmpty,
    /// A file chosen for restore is not a Password365 store/backup.
    #[error("this file is not a Password365 backup")]
    NotABackup,
    /// The device on the other end is not paired with this one.
    #[error("this device is not paired with the other one")]
    UnknownDevice,
    /// A new device claimed the id or key of a device already paired.
    #[error("a device with this identity is already paired; remove it first")]
    AlreadyPaired,
    /// The device was removed from this account.
    #[error("this device was removed from the account")]
    RevokedDevice,
    /// A paired device sent something the sync protocol does not allow.
    #[error("sync failed: {0}")]
    SyncProtocol(&'static str),
    /// The other device's changes are dated in the future (its clock is
    /// ahead): taking them would let them win every conflict until then.
    #[error("the other device's clock is ahead of this one's: check the date and time on both")]
    ClockAhead,
    /// Records were rolled back, removed or added outside Password365.
    #[error(
        "this vault file was changed outside Password365 (records put back, removed or added): restore a backup or set this device up again from another one"
    )]
    Tampered,
    /// A file chosen for import is not a Password365 sync file.
    #[error("this file is not a Password365 sync file, or it is damaged")]
    NotASyncFile,
    /// Pairing data from another device could not be read.
    #[error("the data from the other device could not be read; pair again")]
    NotABundle,
    /// File system error (backup / restore).
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    /// The quick-unlock copy is too old, from another password, or not for
    /// this key: the master password is needed.
    #[error("enter your master password")]
    QuickUnlockExpired,
    /// An update was based on an outdated version of the item.
    #[error("item was modified in the meantime (stale version)")]
    StaleVersion,
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, VaultError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_credentials_message_does_not_say_which() {
        let msg = VaultError::WrongCredentials.to_string();
        assert!(msg.contains("password or Secret Key"));
    }

    #[test]
    fn conversions_and_messages() {
        assert!(matches!(
            VaultError::from(CryptoError::Decrypt),
            VaultError::Crypto(_)
        ));
        assert!(matches!(
            VaultError::from(ModelError::InvalidId),
            VaultError::Model(_)
        ));
        let db = VaultError::from(rusqlite::Error::QueryReturnedNoRows);
        assert!(db.to_string().starts_with("database error"));
        for e in [
            VaultError::AlreadyExists,
            VaultError::NotInitialized,
            VaultError::UnsupportedFormat(9),
            VaultError::Corrupt("x"),
            VaultError::NotFound("item"),
            VaultError::VaultNotEmpty,
            VaultError::StaleVersion,
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
