//! Passkeys: what an authenticator answers to `navigator.credentials.create`
//! (registration) and `navigator.credentials.get` (sign-in).
//!
//! The browser extension only relays: the origin comes from the tab's real
//! URL, and this crate builds `clientDataJSON` itself, so a page cannot claim
//! to be another site. Keys are ECDSA P-256 (ES256); attestation is "none";
//! the signature counter stays 0, as for other synced passkeys (iCloud
//! Keychain, 1Password), since copies on several devices cannot share one.

pub mod authenticator;
pub mod cbor;
pub mod rp;

pub use authenticator::{Assertion, Passkey, Registration};

/// What can go wrong.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WebAuthnError {
    /// The page's origin may not use this relying-party id.
    #[error("this site cannot use passkeys for {0}")]
    WrongSite(String),
    /// The site accepts none of the algorithms supported here (only ES256).
    #[error("the site does not accept this kind of passkey")]
    UnsupportedAlgorithm,
    /// A value from the page or the vault was not readable.
    #[error("invalid passkey data: {0}")]
    Invalid(&'static str),
    /// Key generation failed.
    #[error("cryptography failed: {0}")]
    Crypto(#[from] password365_crypto::CryptoError),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, WebAuthnError>;

/// Base64url without padding (`WebAuthn`'s encoding for binary values in JSON).
#[must_use]
pub fn b64url(bytes: &[u8]) -> String {
    data_encoding::BASE64URL_NOPAD.encode(bytes)
}

/// Reads base64url (with or without padding).
pub fn from_b64url(text: &str) -> Result<Vec<u8>> {
    data_encoding::BASE64URL_NOPAD
        .decode(text.trim_end_matches('=').as_bytes())
        .map_err(|_| WebAuthnError::Invalid("base64url"))
}
