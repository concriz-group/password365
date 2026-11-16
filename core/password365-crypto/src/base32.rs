//! Crockford base32 (no padding), used for human-typed keys.
//!
//! The alphabet excludes I, L, O and U to avoid confusion. Decoding is lenient
//! as the Crockford spec recommends: case-insensitive, `I`/`L` → `1`,
//! `O` → `0`, and hyphens / whitespace are ignored.

use crate::{CryptoError, Result};

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Encodes bytes as Crockford base32 (most significant bit first, no padding).
#[must_use]
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(5) * 8);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    for &byte in data {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(symbol((buffer >> bits) & 0x1F));
        }
    }
    if bits > 0 {
        out.push(symbol((buffer << (5 - bits)) & 0x1F));
    }
    out
}

/// Decodes Crockford base32 into exactly `expected_len` bytes.
///
/// Fails on invalid symbols, wrong length, or non-zero padding bits.
pub fn decode(text: &str, expected_len: usize) -> Result<Vec<u8>> {
    let values: Vec<u32> = canonicalize(text)?
        .chars()
        .map(value_of)
        .collect::<Result<_>>()?;

    if values.len() != (expected_len * 8).div_ceil(5) {
        return Err(CryptoError::Malformed("wrong base32 length"));
    }

    let mut out = Vec::with_capacity(expected_len);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    for value in values {
        buffer = (buffer << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(
                u8::try_from((buffer >> bits) & 0xFF)
                    .map_err(|_| CryptoError::Malformed("base32"))?,
            );
        }
    }
    if buffer & ((1 << bits) - 1) != 0 {
        return Err(CryptoError::Malformed("non-zero base32 padding bits"));
    }
    Ok(out)
}

/// Canonicalizes user-typed base32: drops hyphens/whitespace, upper-cases,
/// maps confusable letters, and validates every symbol.
pub fn canonicalize(text: &str) -> Result<String> {
    text.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'I' | 'L' => Ok('1'),
            'O' => Ok('0'),
            other if ALPHABET.contains(&(other as u8)) && other.is_ascii() => Ok(other),
            _ => Err(CryptoError::Malformed("invalid base32 symbol")),
        })
        .collect()
}

fn symbol(index: u32) -> char {
    char::from(ALPHABET[index as usize])
}

/// Value of an already-canonical symbol.
fn value_of(c: char) -> Result<u32> {
    ALPHABET
        .iter()
        .position(|&a| char::from(a) == c)
        .and_then(|p| u32::try_from(p).ok())
        .ok_or(CryptoError::Malformed("invalid base32 symbol"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_known_values() {
        assert_eq!(encode(&[]), "");
        assert_eq!(encode(&[0]), "00");
        assert_eq!(encode(&[0xFF]), "ZW");
        assert_eq!(encode(&[0xFF; 5]), "ZZZZZZZZ");
        assert_eq!(encode(b"f"), "CR");
    }

    #[test]
    fn round_trips_various_lengths() {
        for len in 0..40 {
            let data: Vec<u8> = (0..len)
                .map(|i| u8::try_from(i * 37 % 256).unwrap())
                .collect();
            assert_eq!(decode(&encode(&data), len).unwrap(), data);
        }
    }

    #[test]
    fn decoding_is_lenient() {
        let canonical = encode(&[0x01, 0x23, 0x45, 0x67, 0x89]);
        let messy = format!("{}-{} ", &canonical[..4], &canonical[4..]).to_lowercase();
        assert_eq!(
            decode(&messy, 5).unwrap(),
            vec![0x01, 0x23, 0x45, 0x67, 0x89]
        );
    }

    #[test]
    fn confusable_letters_are_mapped() {
        assert_eq!(decode("I0", 1).unwrap(), decode("10", 1).unwrap());
        assert_eq!(decode("L0", 1).unwrap(), decode("10", 1).unwrap());
        assert_eq!(decode("0O", 1).unwrap(), decode("00", 1).unwrap());
    }

    #[test]
    fn canonicalize_normalizes_input() {
        assert_eq!(canonicalize("ab-c d").unwrap(), "ABCD");
        assert_eq!(canonicalize("iLo").unwrap(), "110");
        assert!(canonicalize("u").is_err());
        assert!(canonicalize("é").is_err());
    }

    #[test]
    fn rejects_invalid_symbol() {
        assert!(decode("U0", 1).is_err());
        assert!(decode("*0", 1).is_err());
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(decode("000", 1).is_err());
        assert!(decode("0", 1).is_err());
    }

    #[test]
    fn rejects_non_zero_padding_bits() {
        // "01" → last 2 bits of the second symbol are padding and non-zero.
        assert!(decode("01", 1).is_err());
    }
}
