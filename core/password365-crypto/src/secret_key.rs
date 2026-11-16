//! The account Secret Key — Password365's second secret factor.
//!
//! 128 random bits generated once per account and stored only on enrolled
//! devices and the printed Emergency Kit. Because it is combined with the
//! master password during key derivation, a stolen vault file or backup
//! cannot be brute-forced even when the master password is weak.
//!
//! Text format: `Q1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX` — prefix `Q1`
//! (format version 1) followed by 26 base32 symbols of key material and
//! 2 symbols of checksum to catch typos.

use std::fmt;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{CryptoError, Result, base32, random};

/// Number of random bytes in a Secret Key.
pub const SECRET_KEY_LEN: usize = 16;
const PREFIX: &str = "Q1";
const BODY_SYMBOLS: usize = 26;
const CHECK_SYMBOLS: usize = 2;
const GROUP: usize = 4;
const CHECK_DOMAIN: &[u8] = b"quillock/v1/secret-key-check";

/// The account Secret Key. Zeroized on drop; never printed by `Debug`.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct AccountSecretKey([u8; SECRET_KEY_LEN]);

impl AccountSecretKey {
    /// Generates a new random Secret Key.
    pub fn generate() -> Result<Self> {
        random::bytes().map(Self)
    }

    /// Builds a key from raw bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; SECRET_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Raw key bytes.
    #[must_use]
    pub fn expose(&self) -> &[u8; SECRET_KEY_LEN] {
        &self.0
    }

    /// Human-readable form, e.g. for the Emergency Kit.
    #[must_use]
    pub fn to_display_string(&self) -> String {
        let symbols = format!("{}{}", base32::encode(&self.0), checksum(&self.0));
        let groups: Vec<&str> = symbols
            .as_bytes()
            .chunks(GROUP)
            .map(|c| std::str::from_utf8(c).unwrap_or_default())
            .collect();
        format!("{PREFIX}-{}", groups.join("-"))
    }

    /// Parses the human-readable form. Tolerates case, spaces and missing hyphens.
    pub fn parse(text: &str) -> Result<Self> {
        let trimmed = text.trim_start();
        let rest = trimmed
            .get(..PREFIX.len())
            .filter(|p| p.eq_ignore_ascii_case(PREFIX))
            .map(|_| &trimmed[PREFIX.len()..])
            .ok_or(CryptoError::InvalidSecretKey("must start with Q1"))?;
        let symbols = base32::canonicalize(rest)
            .map_err(|_| CryptoError::InvalidSecretKey("invalid characters"))?;
        if symbols.len() != BODY_SYMBOLS + CHECK_SYMBOLS {
            return Err(CryptoError::InvalidSecretKey("wrong length"));
        }
        let (body, check) = symbols.split_at(BODY_SYMBOLS);
        let bytes: [u8; SECRET_KEY_LEN] = base32::decode(body, SECRET_KEY_LEN)
            .map_err(|_| CryptoError::InvalidSecretKey("invalid characters"))?
            .try_into()
            .map_err(|_| CryptoError::InvalidSecretKey("wrong length"))?;

        if !bool::from(checksum(&bytes).as_bytes().ct_eq(check.as_bytes())) {
            return Err(CryptoError::InvalidSecretKey(
                "checksum mismatch — check for typos",
            ));
        }
        Ok(Self(bytes))
    }
}

impl PartialEq for AccountSecretKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for AccountSecretKey {}

impl fmt::Debug for AccountSecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccountSecretKey(<redacted>)")
    }
}

/// Two base32 symbols (10 bits) of a domain-separated SHA-256 over the key.
fn checksum(bytes: &[u8; SECRET_KEY_LEN]) -> String {
    let digest = Sha256::new()
        .chain_update(CHECK_DOMAIN)
        .chain_update(bytes)
        .finalize();
    // First 10 bits → exactly two symbols; mask off the padding bits of the second byte.
    base32::encode(&[digest[0], digest[1] & 0xC0])[..CHECK_SYMBOLS].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_format_shape() {
        let key = AccountSecretKey::from_bytes([0; SECRET_KEY_LEN]);
        let text = key.to_display_string();
        assert!(text.starts_with("Q1-"));
        // Q1 + 7 groups of 4
        assert_eq!(text.split('-').count(), 8);
        assert!(text.split('-').skip(1).all(|g| g.len() == 4));
    }

    #[test]
    fn round_trips_through_text() {
        for _ in 0..50 {
            let key = AccountSecretKey::generate().unwrap();
            assert_eq!(
                AccountSecretKey::parse(&key.to_display_string()).unwrap(),
                key
            );
        }
    }

    #[test]
    fn parse_tolerates_formatting() {
        let key = AccountSecretKey::generate().unwrap();
        let messy = key.to_display_string().replace('-', " ").to_lowercase();
        assert_eq!(AccountSecretKey::parse(&messy).unwrap(), key);
        let compact = key.to_display_string().replace('-', "");
        assert_eq!(AccountSecretKey::parse(&compact).unwrap(), key);
    }

    #[test]
    fn detects_single_symbol_typo() {
        let key = AccountSecretKey::from_bytes([7; SECRET_KEY_LEN]);
        let text = key.to_display_string();
        // Change one symbol in the body.
        let mut chars: Vec<char> = text.chars().collect();
        chars[5] = if chars[5] == 'A' { 'B' } else { 'A' };
        let typo: String = chars.into_iter().collect();
        assert!(AccountSecretKey::parse(&typo).is_err());
    }

    #[test]
    fn rejects_wrong_prefix() {
        let text = AccountSecretKey::generate()
            .unwrap()
            .to_display_string()
            .replacen("Q1", "Q2", 1);
        assert_eq!(
            AccountSecretKey::parse(&text),
            Err(CryptoError::InvalidSecretKey("must start with Q1"))
        );
    }

    #[test]
    fn rejects_wrong_length() {
        assert_eq!(
            AccountSecretKey::parse("Q1-ABCD"),
            Err(CryptoError::InvalidSecretKey("wrong length"))
        );
    }

    #[test]
    fn rejects_invalid_body_characters() {
        let bad = format!("Q1{}{}", "*".repeat(BODY_SYMBOLS), "00");
        assert_eq!(
            AccountSecretKey::parse(&bad),
            Err(CryptoError::InvalidSecretKey("invalid characters"))
        );
    }

    #[test]
    fn rejects_invalid_check_characters() {
        let key = AccountSecretKey::generate().unwrap();
        let text = key.to_display_string();
        let bad = format!("{}**", &text[..text.len() - 2]);
        assert_eq!(
            AccountSecretKey::parse(&bad),
            Err(CryptoError::InvalidSecretKey("invalid characters"))
        );
    }

    #[test]
    fn confusable_checksum_symbols_are_accepted() {
        // Find a key whose checksum contains 1 or 0, then type it as I/O.
        for _ in 0..500 {
            let key = AccountSecretKey::generate().unwrap();
            let text = key.to_display_string();
            let check = &text[text.len() - 2..];
            if check.contains('1') || check.contains('0') {
                let swapped = format!(
                    "{}{}",
                    &text[..text.len() - 2],
                    check.replace('1', "I").replace('0', "O")
                );
                assert_eq!(AccountSecretKey::parse(&swapped).unwrap(), key);
                return;
            }
        }
        panic!("no checksum with 0/1 found in 500 tries");
    }

    #[test]
    fn debug_is_redacted() {
        let key = AccountSecretKey::generate().unwrap();
        assert_eq!(format!("{key:?}"), "AccountSecretKey(<redacted>)");
    }

    #[test]
    fn expose_returns_bytes() {
        assert_eq!(AccountSecretKey::from_bytes([3; 16]).expose(), &[3; 16]);
    }
}
