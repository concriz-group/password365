use password365_crypto::CryptoError;
use thiserror::Error;

/// Generator errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GeneratorError {
    /// The requested recipe cannot produce a valid result.
    #[error("invalid recipe: {0}")]
    InvalidRecipe(String),
    /// The random number generator failed.
    #[error(transparent)]
    Crypto(#[from] CryptoError),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, GeneratorError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages() {
        assert_eq!(
            GeneratorError::InvalidRecipe("x".into()).to_string(),
            "invalid recipe: x"
        );
        assert_eq!(
            GeneratorError::from(CryptoError::Random).to_string(),
            CryptoError::Random.to_string()
        );
    }
}
