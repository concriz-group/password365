//! Cryptographic core of Password365.
//!
//! Every primitive used by the password manager lives here so it can be reviewed,
//! tested and audited in one place. The exact byte formats are specified in
//! `docs/crypto-spec.md`.
//!
//! Module overview:
//! - [`random`]      – OS CSPRNG access and unbiased sampling.
//! - [`key`]         – 256-bit symmetric key type (zeroized on drop).
//! - [`secret`]      – zeroizing string for passwords and other secrets.
//! - [`aad`]         – unambiguous associated-data builder.
//! - [`aead`]        – XChaCha20-Poly1305 sealing / opening.
//! - [`kdf`]         – Argon2id password hashing with validated parameters.
//! - [`hkdf`]        – HKDF-SHA-256 sub-key derivation.
//! - [`base32`]      – Crockford base32 used for human-readable keys.
//! - [`secret_key`]  – the account Secret Key (second factor).
//! - [`unlock`]      – derivation of the Account Unlock Key (AUK).
//! - [`wrap`]        – wrapping one key under another.

pub mod aad;
pub mod aead;
pub mod base32;
mod error;
pub mod hkdf;
pub mod kdf;
pub mod key;
pub mod passkey;
pub mod random;
pub mod secret;
pub mod secret_key;
pub mod totp;
pub mod unlock;
pub mod wrap;

pub use error::{CryptoError, Result};
