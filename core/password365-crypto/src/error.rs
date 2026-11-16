use thiserror::Error;

/// Errors produced by the crypto layer.
///
/// Messages are deliberately vague about *why* decryption failed so callers
/// cannot build oracles from them.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    /// The operating system random number generator failed.
    #[error("system random number generator unavailable")]
    Random,
    /// Authenticated decryption failed (wrong key, tampered data or wrong context).
    #[error("decryption failed")]
    Decrypt,
    /// Encryption failed (should never happen with valid inputs).
    #[error("encryption failed")]
    Encrypt,
    /// Input bytes were not in the expected format.
    #[error("malformed data: {0}")]
    Malformed(&'static str),
    /// KDF parameters are below the security floor or otherwise invalid.
    #[error("invalid key-derivation parameters: {0}")]
    WeakKdfParams(&'static str),
    /// Key derivation failed.
    #[error("key derivation failed")]
    Kdf,
    /// A Secret Key string could not be parsed.
    #[error("invalid Secret Key: {0}")]
    InvalidSecretKey(&'static str),
    /// A one-time password secret or `otpauth://` link could not be read.
    #[error("invalid one-time password setup: {0}")]
    InvalidTotp(&'static str),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, CryptoError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_error_message_reveals_nothing_specific() {
        assert_eq!(CryptoError::Decrypt.to_string(), "decryption failed");
    }

    #[test]
    fn malformed_error_includes_reason() {
        assert_eq!(
            CryptoError::Malformed("too short").to_string(),
            "malformed data: too short"
        );
    }

    #[test]
    fn every_variant_has_a_message() {
        let all = [
            CryptoError::Random,
            CryptoError::Decrypt,
            CryptoError::Encrypt,
            CryptoError::Malformed("x"),
            CryptoError::WeakKdfParams("x"),
            CryptoError::Kdf,
            CryptoError::InvalidSecretKey("x"),
        ];
        for e in all {
            assert!(!e.to_string().is_empty());
        }
    }
}
