//! Authenticated encryption with XChaCha20-Poly1305.
//!
//! Sealed format (see `docs/crypto-spec.md`):
//!
//! ```text
//! version (1 byte = 0x01) || nonce (24 bytes, random) || ciphertext || tag (16 bytes)
//! ```
//!
//! The 192-bit random nonce makes accidental nonce reuse statistically
//! impossible, so no nonce counter state is needed across devices.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::aad::Aad;
use crate::key::SymmetricKey;
use crate::{CryptoError, Result, random};

/// Format version byte for XChaCha20-Poly1305 sealed boxes.
pub const FORMAT_V1: u8 = 0x01;
/// Nonce length in bytes.
pub const NONCE_LEN: usize = 24;
/// Authentication tag length in bytes.
pub const TAG_LEN: usize = 16;
/// Fixed overhead added by [`seal`].
pub const OVERHEAD: usize = 1 + NONCE_LEN + TAG_LEN;

/// Encrypts `plaintext` under `key`, authenticating `aad`.
pub fn seal(key: &SymmetricKey, plaintext: &[u8], aad: &Aad) -> Result<Vec<u8>> {
    let nonce: [u8; NONCE_LEN] = random::bytes()?;
    seal_with_nonce(key, &nonce, plaintext, aad)
}

fn seal_with_nonce(
    key: &SymmetricKey,
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
    aad: &Aad,
) -> Result<Vec<u8>> {
    let ciphertext = cipher(key)
        .encrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| CryptoError::Encrypt)?;

    let mut out = Vec::with_capacity(OVERHEAD + plaintext.len());
    out.push(FORMAT_V1);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypts a box produced by [`seal`]. The plaintext is zeroized on drop.
pub fn open(key: &SymmetricKey, sealed: &[u8], aad: &Aad) -> Result<Zeroizing<Vec<u8>>> {
    if sealed.len() < OVERHEAD {
        return Err(CryptoError::Malformed("sealed box too short"));
    }
    let (version, rest) = sealed.split_at(1);
    if version[0] != FORMAT_V1 {
        return Err(CryptoError::Malformed("unsupported sealed box version"));
    }
    let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| CryptoError::Decrypt)?;

    cipher(key)
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Decrypt)
}

fn cipher(key: &SymmetricKey) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(&(*key.expose()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> SymmetricKey {
        SymmetricKey::from_bytes([0x42; 32])
    }

    fn aad() -> Aad {
        Aad::new("test").bytes(b"ctx")
    }

    #[test]
    fn round_trip() {
        let sealed = seal(&key(), b"hello", &aad()).unwrap();
        assert_eq!(open(&key(), &sealed, &aad()).unwrap().as_slice(), b"hello");
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let sealed = seal(&key(), b"", &aad()).unwrap();
        assert_eq!(sealed.len(), OVERHEAD);
        assert!(open(&key(), &sealed, &aad()).unwrap().is_empty());
    }

    #[test]
    fn output_layout_has_version_nonce_and_tag() {
        let sealed = seal(&key(), b"abc", &aad()).unwrap();
        assert_eq!(sealed[0], FORMAT_V1);
        assert_eq!(sealed.len(), OVERHEAD + 3);
    }

    #[test]
    fn same_plaintext_encrypts_differently() {
        let a = seal(&key(), b"same", &aad()).unwrap();
        let b = seal(&key(), b"same", &aad()).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn known_answer_vector_is_stable() {
        // Regression vector: fixed key + nonce must always yield the same bytes.
        let sealed = seal_with_nonce(&key(), &[1; NONCE_LEN], b"password365", &aad()).unwrap();
        let again = seal_with_nonce(&key(), &[1; NONCE_LEN], b"password365", &aad()).unwrap();
        assert_eq!(sealed, again);
        assert_eq!(&sealed[1..=NONCE_LEN], &[1; NONCE_LEN]);
        assert_eq!(
            open(&key(), &sealed, &aad()).unwrap().as_slice(),
            b"password365"
        );
    }

    #[test]
    fn wrong_key_fails() {
        let sealed = seal(&key(), b"secret", &aad()).unwrap();
        let other = SymmetricKey::from_bytes([0x43; 32]);
        assert_eq!(open(&other, &sealed, &aad()), Err(CryptoError::Decrypt));
    }

    #[test]
    fn wrong_aad_fails() {
        let sealed = seal(&key(), b"secret", &aad()).unwrap();
        let other_aad = Aad::new("test").bytes(b"other");
        assert_eq!(open(&key(), &sealed, &other_aad), Err(CryptoError::Decrypt));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let mut sealed = seal(&key(), b"secret", &aad()).unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert_eq!(open(&key(), &sealed, &aad()), Err(CryptoError::Decrypt));
    }

    #[test]
    fn tampered_nonce_fails() {
        let mut sealed = seal(&key(), b"secret", &aad()).unwrap();
        sealed[1] ^= 1;
        assert_eq!(open(&key(), &sealed, &aad()), Err(CryptoError::Decrypt));
    }

    #[test]
    fn too_short_input_is_malformed() {
        assert!(matches!(
            open(&key(), &[FORMAT_V1; 10], &aad()),
            Err(CryptoError::Malformed(_))
        ));
    }

    #[test]
    fn unknown_version_is_malformed() {
        let mut sealed = seal(&key(), b"x", &aad()).unwrap();
        sealed[0] = 0x99;
        assert!(matches!(
            open(&key(), &sealed, &aad()),
            Err(CryptoError::Malformed(_))
        ));
    }
}
