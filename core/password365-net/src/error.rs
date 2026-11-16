//! Transport errors.

use password365_sync::pairing::{InviteError, PairingError};
use password365_vault::VaultError;

/// Why a connection or sync failed.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    /// Network or socket problem (including timeouts and a closed connection).
    #[error("connection problem: {0}")]
    Io(#[from] std::io::Error),
    /// The encrypted channel could not be set up or was tampered with.
    #[error(transparent)]
    Channel(#[from] PairingError),
    /// The store refused the exchange (unknown device, protocol violation, ...).
    #[error(transparent)]
    Vault(#[from] VaultError),
    /// The scanned code is not a usable pairing code.
    #[error(transparent)]
    Invite(#[from] InviteError),
    /// The pairing code carries no network address to connect to.
    #[error("the other device is not reachable on the network")]
    NoAddress,
    /// The other side cancelled pairing.
    #[error("pairing was cancelled on the other device")]
    Cancelled,
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, NetError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_well() {
        let io = NetError::from(std::io::Error::other("reset"));
        assert_eq!(io.to_string(), "connection problem: reset");
        assert!(
            NetError::from(PairingError::TooLarge)
                .to_string()
                .contains("too large")
        );
        assert!(
            NetError::from(VaultError::UnknownDevice)
                .to_string()
                .contains("not paired")
        );
    }
}
