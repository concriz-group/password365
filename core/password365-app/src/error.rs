//! Application errors with stable machine-readable codes for frontends.

use password365_crypto::CryptoError;
use password365_generator::GeneratorError;
use password365_import::ImportError;
use password365_model::ModelError;
use password365_vault::VaultError;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use thiserror::Error;

/// Errors returned to frontends.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppError {
    /// An operation needs an unlocked vault.
    #[error("the vault is locked")]
    Locked,
    /// No account has been created yet.
    #[error("no account exists yet")]
    NotInitialized,
    /// An account already exists.
    #[error("an account already exists")]
    AlreadyInitialized,
    /// Wrong master password or Secret Key.
    #[error("wrong master password or Secret Key")]
    WrongCredentials,
    /// Something referenced does not exist.
    #[error("{0} not found")]
    NotFound(&'static str),
    /// The item changed since it was loaded.
    #[error("the item was changed elsewhere — reload and try again")]
    Conflict,
    /// User input is invalid.
    #[error("{0}")]
    Validation(String),
    /// File system problem.
    #[error("{0}")]
    Io(String),
    /// Unexpected internal failure.
    #[error("internal error: {0}")]
    Internal(String),
    /// This device does not have the Secret Key (new device, or forgotten
    /// after too many wrong passwords): type it from the Emergency Kit.
    #[error("enter your Secret Key (from the Emergency Kit) with your master password")]
    SecretKeyRequired,
    /// Quick unlock (Face ID) is not possible any more: type the master password.
    #[error("enter your master password")]
    PasswordRequired,
    /// The site wants the user verified and the vault was not unlocked
    /// recently: type the master password to go on.
    #[error("confirm it is you: type your master password")]
    VerificationNeeded,
    /// Too many wrong master passwords: wait this many seconds.
    #[error("too many wrong passwords — try again in {0} s")]
    TooManyAttempts(u64),
}

impl AppError {
    /// Stable identifier frontends can switch on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Locked => "locked",
            Self::NotInitialized => "not_initialized",
            Self::AlreadyInitialized => "already_initialized",
            Self::WrongCredentials => "wrong_credentials",
            Self::NotFound(_) => "not_found",
            Self::Conflict => "conflict",
            Self::Validation(_) => "validation",
            Self::Io(_) => "io",
            Self::Internal(_) => "internal",
            Self::TooManyAttempts(_) => "too_many_attempts",
            Self::SecretKeyRequired => "secret_key_required",
            Self::PasswordRequired => "password_required",
            Self::VerificationNeeded => "verification_needed",
        }
    }
}

/// Serialized as `{ "code": "...", "message": "..." }`.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<VaultError> for AppError {
    fn from(e: VaultError) -> Self {
        match e {
            VaultError::WrongCredentials => Self::WrongCredentials,
            VaultError::NotFound(what) => Self::NotFound(what),
            VaultError::AlreadyExists => Self::AlreadyInitialized,
            VaultError::NotInitialized => Self::NotInitialized,
            VaultError::StaleVersion => Self::Conflict,
            VaultError::Model(m) => m.into(),
            VaultError::VaultNotEmpty
            | VaultError::NotABackup
            | VaultError::NotABundle
            | VaultError::NotASyncFile
            | VaultError::ClockAhead
            | VaultError::Tampered
            | VaultError::AlreadyPaired
            | VaultError::RevokedDevice => Self::Validation(e.to_string()),
            VaultError::Io(io) => Self::Io(io.to_string()),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<ModelError> for AppError {
    fn from(e: ModelError) -> Self {
        Self::Validation(e.to_string())
    }
}

impl From<CryptoError> for AppError {
    fn from(e: CryptoError) -> Self {
        match e {
            CryptoError::InvalidSecretKey(_) | CryptoError::InvalidTotp(_) => {
                Self::Validation(e.to_string())
            }
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<GeneratorError> for AppError {
    fn from(e: GeneratorError) -> Self {
        match e {
            GeneratorError::InvalidRecipe(_) => Self::Validation(e.to_string()),
            GeneratorError::Crypto(c) => c.into(),
        }
    }
}

impl From<ImportError> for AppError {
    fn from(e: ImportError) -> Self {
        Self::Validation(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_message() {
        let json = serde_json::to_string(&AppError::Locked).unwrap();
        assert_eq!(json, r#"{"code":"locked","message":"the vault is locked"}"#);
    }

    #[test]
    fn every_variant_has_code_and_message() {
        let all = [
            AppError::Locked,
            AppError::NotInitialized,
            AppError::AlreadyInitialized,
            AppError::WrongCredentials,
            AppError::NotFound("item"),
            AppError::VerificationNeeded,
            AppError::Conflict,
            AppError::Validation("v".into()),
            AppError::Io("i".into()),
            AppError::Internal("x".into()),
        ];
        let codes: std::collections::HashSet<_> = all.iter().map(AppError::code).collect();
        assert_eq!(codes.len(), all.len(), "codes must be unique");
        assert!(all.iter().all(|e| !e.to_string().is_empty()));
    }

    #[test]
    fn maps_vault_errors() {
        assert_eq!(
            AppError::from(VaultError::WrongCredentials),
            AppError::WrongCredentials
        );
        assert_eq!(
            AppError::from(VaultError::NotFound("item")),
            AppError::NotFound("item")
        );
        assert_eq!(
            AppError::from(VaultError::AlreadyExists),
            AppError::AlreadyInitialized
        );
        assert_eq!(
            AppError::from(VaultError::NotInitialized),
            AppError::NotInitialized
        );
        assert_eq!(AppError::from(VaultError::StaleVersion), AppError::Conflict);
        assert_eq!(
            AppError::from(VaultError::VaultNotEmpty).code(),
            "validation"
        );
        assert_eq!(
            AppError::from(VaultError::Model(ModelError::InvalidId)).code(),
            "validation"
        );
        assert_eq!(AppError::from(VaultError::Corrupt("x")).code(), "internal");
        assert_eq!(AppError::from(VaultError::NotABackup).code(), "validation");
        assert_eq!(AppError::from(VaultError::NotABundle).code(), "validation");
        assert_eq!(
            AppError::from(VaultError::NotASyncFile).code(),
            "validation"
        );
        assert_eq!(
            AppError::from(VaultError::Io(std::io::Error::other("disk full"))).code(),
            "io"
        );
    }

    #[test]
    fn maps_other_errors() {
        assert_eq!(
            AppError::from(CryptoError::InvalidSecretKey("x")).code(),
            "validation"
        );
        assert_eq!(AppError::from(CryptoError::Random).code(), "internal");
        assert_eq!(
            AppError::from(GeneratorError::InvalidRecipe("x".into())).code(),
            "validation"
        );
        assert_eq!(
            AppError::from(GeneratorError::Crypto(CryptoError::Random)).code(),
            "internal"
        );
        assert_eq!(
            AppError::from(ImportError::MissingColumn("password")).code(),
            "validation"
        );
        assert_eq!(AppError::from(std::io::Error::other("disk")).code(), "io");
    }
}
