//! Time-based one-time passwords (RFC 6238), the six-digit codes of
//! two-step verification.
//!
//! The stored secret is either a base32 key as sites show it (spaces, case
//! and padding are ignored) or an `otpauth://totp/...` link from a QR code,
//! which may also set the algorithm, the number of digits and the period.

use hmac::{Hmac, KeyInit, Mac};
use zeroize::Zeroizing;

use crate::{CryptoError, Result};

/// HMAC of the big-endian counter with hash `$hash`.
macro_rules! hmac {
    ($hash:ty, $key:expr, $counter:expr) => {{
        let mut mac =
            <Hmac<$hash> as KeyInit>::new_from_slice($key).expect("HMAC takes any key length");
        mac.update(&$counter.to_be_bytes());
        mac.finalize().into_bytes().to_vec()
    }};
}

/// Hash used by the HMAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// SHA-1, what nearly every site uses.
    Sha1,
    /// SHA-256.
    Sha256,
    /// SHA-512.
    Sha512,
}

/// A parsed one-time password setup.
pub struct Totp {
    secret: Zeroizing<Vec<u8>>,
    algorithm: Algorithm,
    digits: u32,
    period: u64,
}

/// A code and how long it stays valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    /// The code, zero-padded (e.g. `"042137"`).
    pub code: String,
    /// Seconds until the next code.
    pub remaining: u64,
    /// Seconds each code lasts.
    pub period: u64,
}

impl Totp {
    /// Reads a base32 secret or an `otpauth://totp/` link.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.trim();
        // Compare bytes, not a string slice: pasted text may have a multi-byte character there.
        if text
            .as_bytes()
            .get(..10)
            .is_some_and(|p| p.eq_ignore_ascii_case(b"otpauth://"))
        {
            return Self::parse_uri(text);
        }
        Self::new(decode_secret(text)?, Algorithm::Sha1, 6, 30)
    }

    /// The secret in standard Base32 (RFC 4648, no padding), for export.
    #[must_use]
    pub fn secret_base32(&self) -> String {
        data_encoding::BASE32_NOPAD.encode(&self.secret)
    }

    /// `sha1`, `sha256` or `sha512`.
    #[must_use]
    pub const fn algorithm_name(&self) -> &'static str {
        match self.algorithm {
            Algorithm::Sha1 => "sha1",
            Algorithm::Sha256 => "sha256",
            Algorithm::Sha512 => "sha512",
        }
    }

    /// Number of digits in a code.
    #[must_use]
    pub const fn digits(&self) -> u32 {
        self.digits
    }

    /// Seconds each code is valid.
    #[must_use]
    pub const fn period(&self) -> u64 {
        self.period
    }

    /// The code at `unix_seconds`.
    #[must_use]
    pub fn code_at(&self, unix_seconds: u64) -> Code {
        let counter = unix_seconds / self.period;
        let mac = match self.algorithm {
            Algorithm::Sha1 => hmac!(sha1::Sha1, &self.secret, counter),
            Algorithm::Sha256 => hmac!(sha2::Sha256, &self.secret, counter),
            Algorithm::Sha512 => hmac!(sha2::Sha512, &self.secret, counter),
        };
        // Dynamic truncation (RFC 4226 §5.3).
        let offset = usize::from(mac[mac.len() - 1] & 0x0f);
        let value = u32::from_be_bytes([
            mac[offset] & 0x7f,
            mac[offset + 1],
            mac[offset + 2],
            mac[offset + 3],
        ]);
        let code = value % 10u32.pow(self.digits);
        Code {
            code: format!("{code:0width$}", width = self.digits as usize),
            remaining: self.period - unix_seconds % self.period,
            period: self.period,
        }
    }

    fn new(
        secret: Zeroizing<Vec<u8>>,
        algorithm: Algorithm,
        digits: u32,
        period: u64,
    ) -> Result<Self> {
        if secret.is_empty() {
            return Err(CryptoError::InvalidTotp("empty secret"));
        }
        if !(6..=8).contains(&digits) {
            return Err(CryptoError::InvalidTotp("digits must be 6 to 8"));
        }
        if !(1..=300).contains(&period) {
            return Err(CryptoError::InvalidTotp("period must be 1 to 300 seconds"));
        }
        Ok(Self {
            secret,
            algorithm,
            digits,
            period,
        })
    }

    fn parse_uri(uri: &str) -> Result<Self> {
        let rest = &uri["otpauth://".len()..];
        let Some((kind, rest)) = rest.split_once('/') else {
            return Err(CryptoError::InvalidTotp("not an otpauth link"));
        };
        if !kind.eq_ignore_ascii_case("totp") {
            return Err(CryptoError::InvalidTotp(
                "only time-based codes are supported",
            ));
        }
        let query = rest.split_once('?').map_or("", |(_, q)| q);
        let (mut secret, mut algorithm, mut digits, mut period) = (None, Algorithm::Sha1, 6, 30);
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let value = Zeroizing::new(percent_decode(value)?);
            match key.to_ascii_lowercase().as_str() {
                "secret" => secret = Some(decode_secret(&value)?),
                "algorithm" => {
                    algorithm = match value.to_ascii_uppercase().as_str() {
                        "SHA1" => Algorithm::Sha1,
                        "SHA256" => Algorithm::Sha256,
                        "SHA512" => Algorithm::Sha512,
                        _ => return Err(CryptoError::InvalidTotp("unknown algorithm")),
                    }
                }
                "digits" => digits = number(&value)?,
                "period" => period = number(&value)?,
                _ => {}
            }
        }
        let secret = secret.ok_or(CryptoError::InvalidTotp("the link has no secret"))?;
        Self::new(secret, algorithm, digits, period)
    }
}

fn decode_secret(text: &str) -> Result<Zeroizing<Vec<u8>>> {
    let cleaned: Zeroizing<String> = Zeroizing::new(
        text.chars()
            .filter(|c| !c.is_whitespace() && *c != '-' && *c != '=')
            .map(|c| c.to_ascii_uppercase())
            .collect(),
    );
    data_encoding::BASE32_NOPAD
        .decode(cleaned.as_bytes())
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::InvalidTotp("the secret is not valid base32"))
}

fn number<T: std::str::FromStr>(text: &str) -> Result<T> {
    text.parse()
        .map_err(|_| CryptoError::InvalidTotp("not a number"))
}

fn percent_decode(text: &str) -> Result<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = text
                    .get(i + 1..i + 3)
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                    .ok_or(CryptoError::InvalidTotp("bad escape in link"))?;
                out.push(hex);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| CryptoError::InvalidTotp("bad text in link"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_with_multibyte_characters_is_refused_not_a_crash() {
        assert!(Totp::parse("ABCDEFGHIౙ").is_err());
        assert!(Totp::parse("otpauth:/ቋx").is_err());
    }

    /// RFC 6238 appendix B seeds, as base32.
    fn seed(len: usize) -> String {
        let ascii: Vec<u8> = b"1234567890".iter().copied().cycle().take(len).collect();
        data_encoding::BASE32_NOPAD.encode(&ascii)
    }

    fn uri(len: usize, algorithm: &str) -> String {
        format!(
            "otpauth://totp/Example:alice@example.com?secret={}&algorithm={algorithm}&digits=8&issuer=Example",
            seed(len)
        )
    }

    #[test]
    fn rfc_6238_test_vectors() {
        let cases = [
            (59, "94287082", "46119246", "90693936"),
            (1_111_111_109, "07081804", "68084774", "25091201"),
            (1_111_111_111, "14050471", "67062674", "99943326"),
            (1_234_567_890, "89005924", "91819424", "93441116"),
            (2_000_000_000, "69279037", "90698825", "38618901"),
            (20_000_000_000, "65353130", "77737706", "47863826"),
        ];
        let sha1 = Totp::parse(&uri(20, "SHA1")).unwrap();
        let sha256 = Totp::parse(&uri(32, "SHA256")).unwrap();
        let sha512 = Totp::parse(&uri(64, "sha512")).unwrap();
        for (t, a, b, c) in cases {
            assert_eq!(sha1.code_at(t).code, a, "SHA1 at {t}");
            assert_eq!(sha256.code_at(t).code, b, "SHA256 at {t}");
            assert_eq!(sha512.code_at(t).code, c, "SHA512 at {t}");
        }
    }

    #[test]
    fn a_plain_secret_gives_six_digits_every_30_seconds() {
        let spaced = "gezd gnbv gy3t qojq gezd gnbv gy3t qojq";
        let totp = Totp::parse(spaced).unwrap();
        let code = totp.code_at(59);
        assert_eq!(code.code, "287082", "last six digits of the RFC vector");
        assert_eq!((code.remaining, code.period), (1, 30));
        assert_eq!(totp.code_at(60).remaining, 30);
        assert!(
            Totp::parse(&format!("{}====", seed(20))).is_ok(),
            "padding ignored"
        );
    }

    #[test]
    fn link_options_are_read() {
        let link = format!(
            "OTPAUTH://TOTP/A%20B?SECRET={}&period=60&digits=7&issuer=A+B",
            seed(20)
        );
        let code = Totp::parse(&link).unwrap().code_at(59);
        assert_eq!((code.code.len(), code.period, code.remaining), (7, 60, 1));
    }

    #[test]
    fn bad_setups_are_refused() {
        for bad in [
            "",
            "not base32!",
            "otpauth://hotp/x?secret=GEZDGNBV",
            "otpauth://totp/x?issuer=nobody",
            "otpauth://totp/x?secret=GEZDGNBV&digits=9",
            "otpauth://totp/x?secret=GEZDGNBV&period=0",
            "otpauth://totp/x?secret=GEZDGNBV&algorithm=MD5",
            "otpauth://totp/x?secret=GEZDGNBV&digits=six",
            "otpauth://totp/x?secret=%zz",
            "otpauth://totp/x?secret=%ff",
            "otpauth://totp",
        ] {
            assert!(
                matches!(Totp::parse(bad), Err(CryptoError::InvalidTotp(_))),
                "{bad:?} should be refused"
            );
        }
    }
}
