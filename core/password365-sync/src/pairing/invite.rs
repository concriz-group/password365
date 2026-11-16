//! The pairing invite an existing device shows as a QR code.
//!
//! Text form: `PASSWORD365-PAIR:1:<BASE32>`. Uppercase base32 fits the QR
//! "alphanumeric" mode, so the code stays small. The payload holds the
//! inviter's device id, its static Noise key, a one-time 256-bit pre-shared key
//! and, optionally, its LAN address. The pre-shared key is what lets only the
//! person who scanned the code take part in the handshake.

use std::fmt;

use data_encoding::BASE32_NOPAD;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::DeviceId;

const PREFIX: &str = "PASSWORD365-PAIR:1:";
const FIXED_LEN: usize = 16 + 32 + 32;
/// Longest LAN address accepted (`[ipv6]:port` fits easily).
pub const MAX_ADDR_LEN: usize = 64;

/// Why an invite could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InviteError {
    /// Not a Password365 pairing code, or an unsupported version.
    #[error("this is not a Password365 pairing code")]
    NotAnInvite,
    /// Right prefix, damaged content.
    #[error("the pairing code is damaged; scan it again")]
    Malformed,
}

/// A one-time pairing invite.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct Invite {
    /// Inviting device.
    #[zeroize(skip)]
    pub device_id: DeviceId,
    /// Inviter's static X25519 key for the Noise handshake.
    pub static_public: [u8; 32],
    /// One-time pre-shared key (secret: only the QR code carries it).
    pub psk: [u8; 32],
    /// `host:port` on the local network, if listening.
    pub addr: Option<String>,
}

impl Invite {
    /// Creates an invite with a fresh random pre-shared key.
    ///
    /// # Panics
    /// If the OS random generator fails (nothing secure can be done then).
    #[must_use]
    pub fn new(device_id: DeviceId, static_public: [u8; 32], addr: Option<String>) -> Self {
        let mut psk = [0u8; 32];
        getrandom::fill(&mut psk).expect("OS random generator");
        Self {
            device_id,
            static_public,
            psk,
            addr,
        }
    }

    /// The text to put in the QR code.
    #[must_use]
    pub fn to_code(&self) -> String {
        let addr = self.addr.as_deref().unwrap_or("").as_bytes();
        let mut bytes = Vec::with_capacity(FIXED_LEN + addr.len());
        bytes.extend_from_slice(self.device_id.as_bytes());
        bytes.extend_from_slice(&self.static_public);
        bytes.extend_from_slice(&self.psk);
        bytes.extend_from_slice(addr);
        let code = format!("{PREFIX}{}", BASE32_NOPAD.encode(&bytes));
        bytes.zeroize();
        code
    }

    /// Reads a scanned (or typed) code. Case and surrounding spaces are ignored.
    pub fn parse(code: &str) -> Result<Self, InviteError> {
        let code = code.trim().to_ascii_uppercase();
        let body = code.strip_prefix(PREFIX).ok_or(InviteError::NotAnInvite)?;
        let mut bytes = BASE32_NOPAD
            .decode(body.as_bytes())
            .map_err(|_| InviteError::Malformed)?;
        let invite = Self::from_bytes(&bytes);
        bytes.zeroize();
        invite
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, InviteError> {
        if bytes.len() < FIXED_LEN || bytes.len() > FIXED_LEN + MAX_ADDR_LEN {
            return Err(InviteError::Malformed);
        }
        let (fixed, addr) = bytes.split_at(FIXED_LEN);
        let take = |range: std::ops::Range<usize>| -> [u8; 32] {
            let mut out = [0u8; 32];
            out.copy_from_slice(&fixed[range]);
            out
        };
        let mut id = [0u8; 16];
        id.copy_from_slice(&fixed[..16]);
        let addr = std::str::from_utf8(addr).map_err(|_| InviteError::Malformed)?;
        Ok(Self {
            device_id: DeviceId::from_bytes(id),
            static_public: take(16..48),
            psk: take(48..80),
            addr: (!addr.is_empty()).then(|| addr.to_owned()),
        })
    }
}

impl fmt::Debug for Invite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Invite")
            .field("device_id", &self.device_id)
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invite(addr: Option<&str>) -> Invite {
        Invite::new(
            DeviceId::from_bytes([7; 16]),
            [9; 32],
            addr.map(str::to_owned),
        )
    }

    #[test]
    fn round_trips_through_the_qr_text() {
        for addr in [None, Some("192.168.1.20:7480"), Some("[fe80::1]:7480")] {
            let original = invite(addr);
            let code = original.to_code();
            assert!(code.starts_with(PREFIX));
            assert!(
                code.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == ':' || c == '-')
            );
            assert_eq!(Invite::parse(&code).unwrap(), original);
        }
    }

    #[test]
    fn parsing_ignores_case_and_spaces() {
        let original = invite(None);
        let code = format!("  {}\n", original.to_code().to_lowercase());
        assert_eq!(Invite::parse(&code).unwrap(), original);
    }

    #[test]
    fn every_invite_has_a_fresh_secret() {
        assert_ne!(invite(None).psk, invite(None).psk);
    }

    #[test]
    fn foreign_and_damaged_codes_are_rejected() {
        assert_eq!(
            Invite::parse("https://example.com"),
            Err(InviteError::NotAnInvite)
        );
        assert_eq!(
            Invite::parse("PASSWORD365-PAIR:2:AAAA"),
            Err(InviteError::NotAnInvite)
        );
        assert_eq!(
            Invite::parse("PASSWORD365-PAIR:1:!!!"),
            Err(InviteError::Malformed)
        );
        assert_eq!(
            Invite::parse("PASSWORD365-PAIR:1:AAAA"),
            Err(InviteError::Malformed)
        );
        let long = Invite {
            addr: Some("x".repeat(MAX_ADDR_LEN + 1)),
            ..invite(None)
        };
        assert_eq!(Invite::parse(&long.to_code()), Err(InviteError::Malformed));
        let mut bytes = vec![0u8; FIXED_LEN];
        bytes.push(0xff);
        let bad_utf8 = format!("{PREFIX}{}", BASE32_NOPAD.encode(&bytes));
        assert_eq!(Invite::parse(&bad_utf8), Err(InviteError::Malformed));
    }

    #[test]
    fn debug_never_shows_the_secret() {
        let shown = format!("{:?}", invite(Some("10.0.0.2:1")));
        assert!(shown.contains("10.0.0.2"));
        assert!(!shown.contains("psk"));
        assert!(!shown.contains("static_public"));
    }

    #[test]
    fn errors_read_well() {
        assert_eq!(
            InviteError::NotAnInvite.to_string(),
            "this is not a Password365 pairing code"
        );
        assert!(InviteError::Malformed.to_string().contains("scan it again"));
    }
}
