//! The encrypted pairing channel: Noise `XXpsk3` (X25519, ChaCha20-Poly1305,
//! BLAKE2s) keyed with the invite's one-time secret.
//!
//! Only someone who scanned the QR code knows the pre-shared key, and the
//! scanner checks that the inviter's static key matches the one in the code.
//! As a last check against a code seen over someone's shoulder, both screens
//! show a 6-digit code derived from the handshake, and the user confirms they
//! match. Transport-agnostic: messages are plain byte vectors.

use std::fmt;

use sha2::{Digest, Sha256};
use snow::{Builder, HandshakeState, TransportState};
use zeroize::Zeroizing;

use super::invite::Invite;

const PATTERN: &str = "Noise_XXpsk3_25519_ChaChaPoly_BLAKE2s";
const PROLOGUE: &[u8] = b"quillock/v1/pairing";
/// Paired devices: both static keys are exchanged and then checked against
/// the pinned keys of the paired devices (no pre-shared secret needed).
const SYNC_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const SYNC_PROLOGUE: &[u8] = b"quillock/v1/sync";
/// Sync files: one-way, encrypted to the recipient's pinned key and
/// authenticated by the sender's key.
const FILE_PATTERN: &str = "Noise_X_25519_ChaChaPoly_BLAKE2s";
const FILE_PROLOGUE: &[u8] = b"quillock/v1/file";
const PSK_POSITION: u8 = 3;
/// Largest Noise message.
const MAX_MESSAGE: usize = 65_535;
const TAG_LEN: usize = 16;
/// Largest plaintext one [`Channel::seal`] call accepts.
pub const MAX_PLAINTEXT: usize = MAX_MESSAGE - TAG_LEN;

/// What can go wrong while pairing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairingError {
    /// Wrong or expired code, tampering, or a message out of order.
    #[error("pairing failed: the code is wrong or expired, or the connection was tampered with")]
    Handshake,
    /// The device that answered is not the one that showed the code.
    #[error("pairing failed: an unexpected device answered")]
    UnexpectedDevice,
    /// A message bigger than the channel allows.
    #[error("message too large")]
    TooLarge,
}

impl From<snow::Error> for PairingError {
    fn from(_: snow::Error) -> Self {
        Self::Handshake
    }
}

/// A device's long-term X25519 key for Noise.
pub struct StaticKey {
    private: Zeroizing<[u8; 32]>,
    public: [u8; 32],
}

impl StaticKey {
    /// A fresh random key.
    pub fn generate() -> Result<Self, PairingError> {
        let pair = Builder::new(params(PATTERN)).generate_keypair()?;
        let private = Zeroizing::new(to_array(&pair.private)?);
        Ok(Self {
            private,
            public: to_array(&pair.public)?,
        })
    }

    /// Rebuilds a stored key.
    #[must_use]
    pub fn from_parts(private: [u8; 32], public: [u8; 32]) -> Self {
        Self {
            private: Zeroizing::new(private),
            public,
        }
    }

    /// The public half (shared freely).
    #[must_use]
    pub const fn public(&self) -> [u8; 32] {
        self.public
    }

    /// The private half (only to persist it, encrypted).
    #[must_use]
    pub fn private(&self) -> &[u8; 32] {
        &self.private
    }
}

impl fmt::Debug for StaticKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StaticKey").finish_non_exhaustive()
    }
}

/// A pairing handshake in progress (three messages: scanner, inviter, scanner).
pub struct Handshake {
    state: HandshakeState,
    expected_remote: Option<[u8; 32]>,
}

impl Handshake {
    /// The device that scanned the code; it speaks first.
    pub fn scanner(local: &StaticKey, invite: &Invite) -> Result<Self, PairingError> {
        let state = builder(local, invite)?.build_initiator()?;
        Ok(Self {
            state,
            expected_remote: Some(invite.static_public),
        })
    }

    /// The device that showed the code.
    pub fn inviter(local: &StaticKey, invite: &Invite) -> Result<Self, PairingError> {
        let state = builder(local, invite)?.build_responder()?;
        Ok(Self {
            state,
            expected_remote: None,
        })
    }

    /// A sync connection to an already paired device (this side connects).
    /// The caller must check [`Channel::remote_static`] against its peers.
    pub fn sync_initiator(local: &StaticKey) -> Result<Self, PairingError> {
        let state = sync_builder(local)?.build_initiator()?;
        Ok(Self {
            state,
            expected_remote: None,
        })
    }

    /// A sync connection from an already paired device (this side listens).
    pub fn sync_responder(local: &StaticKey) -> Result<Self, PairingError> {
        let state = sync_builder(local)?.build_responder()?;
        Ok(Self {
            state,
            expected_remote: None,
        })
    }

    /// Writes a sync file for the device whose key is `recipient` (one message,
    /// then transport messages in one direction only).
    pub fn file_sender(local: &StaticKey, recipient: &[u8; 32]) -> Result<Self, PairingError> {
        let state = Builder::new(params(FILE_PATTERN))
            .prologue(FILE_PROLOGUE)?
            .local_private_key(local.private())?
            .remote_public_key(recipient)?
            .build_initiator()?;
        Ok(Self {
            state,
            expected_remote: Some(*recipient),
        })
    }

    /// Reads a sync file addressed to this device. The caller checks
    /// [`Channel::remote_static`] (the sender) against its peers.
    pub fn file_receiver(local: &StaticKey) -> Result<Self, PairingError> {
        let state = Builder::new(params(FILE_PATTERN))
            .prologue(FILE_PROLOGUE)?
            .local_private_key(local.private())?
            .build_responder()?;
        Ok(Self {
            state,
            expected_remote: None,
        })
    }

    /// `true` when this side must send the next message.
    #[must_use]
    pub fn is_my_turn(&self) -> bool {
        self.state.is_my_turn()
    }

    /// `true` once all three messages went through.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.state.is_handshake_finished()
    }

    /// Produces the next handshake message.
    pub fn write(&mut self) -> Result<Vec<u8>, PairingError> {
        let mut buf = vec![0u8; MAX_MESSAGE];
        let len = self.state.write_message(&[], &mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Consumes the peer's handshake message.
    pub fn read(&mut self, message: &[u8]) -> Result<(), PairingError> {
        let mut buf = vec![0u8; MAX_MESSAGE];
        self.state.read_message(message, &mut buf)?;
        Ok(())
    }

    /// Completes the handshake: checks who answered and opens the channel.
    pub fn finish(self) -> Result<Channel, PairingError> {
        if !self.is_finished() {
            return Err(PairingError::Handshake);
        }
        let remote = to_array(
            self.state
                .get_remote_static()
                .ok_or(PairingError::Handshake)?,
        )?;
        if self
            .expected_remote
            .is_some_and(|expected| expected != remote)
        {
            return Err(PairingError::UnexpectedDevice);
        }
        let code = verification_code(self.state.get_handshake_hash());
        Ok(Channel {
            transport: self.state.into_transport_mode()?,
            remote_static: remote,
            verification_code: code,
        })
    }
}

/// An established, encrypted pairing channel.
pub struct Channel {
    transport: TransportState,
    remote_static: [u8; 32],
    verification_code: String,
}

impl Channel {
    /// Six digits (`"123 456"`) both screens show; the user confirms they match.
    #[must_use]
    pub fn verification_code(&self) -> &str {
        &self.verification_code
    }

    /// The peer's static key (becomes its identity once confirmed).
    #[must_use]
    pub const fn remote_static(&self) -> [u8; 32] {
        self.remote_static
    }

    /// Encrypts one message for the peer.
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, PairingError> {
        if plaintext.len() > MAX_PLAINTEXT {
            return Err(PairingError::TooLarge);
        }
        let mut buf = vec![0u8; plaintext.len() + TAG_LEN];
        let len = self.transport.write_message(plaintext, &mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Decrypts one message from the peer (fails if tampered, replayed or reordered).
    pub fn open(&mut self, message: &[u8]) -> Result<Zeroizing<Vec<u8>>, PairingError> {
        if message.len() > MAX_MESSAGE {
            return Err(PairingError::TooLarge);
        }
        let mut buf = Zeroizing::new(vec![0u8; message.len()]);
        let len = self.transport.read_message(message, &mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }
}

fn params(pattern: &str) -> snow::params::NoiseParams {
    // The patterns are constants known to parse.
    #[allow(clippy::unwrap_used)]
    pattern.parse().unwrap()
}

fn builder<'a>(local: &'a StaticKey, invite: &'a Invite) -> Result<Builder<'a>, PairingError> {
    Ok(Builder::new(params(PATTERN))
        .prologue(PROLOGUE)?
        .local_private_key(local.private())?
        .psk(PSK_POSITION, &invite.psk)?)
}

fn sync_builder(local: &StaticKey) -> Result<Builder<'_>, PairingError> {
    Ok(Builder::new(params(SYNC_PATTERN))
        .prologue(SYNC_PROLOGUE)?
        .local_private_key(local.private())?)
}

/// Six decimal digits from the handshake hash, grouped `"ddd ddd"`.
fn verification_code(handshake_hash: &[u8]) -> String {
    let digest = Sha256::new()
        .chain_update(b"quillock/v1/verification-code")
        .chain_update(handshake_hash)
        .finalize();
    let n = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

fn to_array(bytes: &[u8]) -> Result<[u8; 32], PairingError> {
    bytes.try_into().map_err(|_| PairingError::Handshake)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeviceId;

    struct Sides {
        scanner: Handshake,
        inviter: Handshake,
    }

    fn sides(inviter_key: &StaticKey, invite: &Invite, scanned: &Invite) -> Sides {
        Sides {
            scanner: Handshake::scanner(&StaticKey::generate().unwrap(), scanned).unwrap(),
            inviter: Handshake::inviter(inviter_key, invite).unwrap(),
        }
    }

    fn run(mut s: Sides) -> (Result<Channel, PairingError>, Result<Channel, PairingError>) {
        let step = |from: &mut Handshake, to: &mut Handshake| -> Result<(), PairingError> {
            assert!(from.is_my_turn());
            let msg = from.write()?;
            to.read(&msg)
        };
        let outcome = step(&mut s.scanner, &mut s.inviter)
            .and_then(|()| step(&mut s.inviter, &mut s.scanner))
            .and_then(|()| step(&mut s.scanner, &mut s.inviter));
        match outcome {
            Ok(()) => (s.scanner.finish(), s.inviter.finish()),
            Err(e) => (Err(e.clone()), Err(e)),
        }
    }

    fn invite_for(key: &StaticKey) -> Invite {
        Invite::new(DeviceId::new(), key.public(), None)
    }

    #[test]
    fn both_sides_see_the_same_code_and_can_talk() {
        let inviter_key = StaticKey::generate().unwrap();
        let invite = invite_for(&inviter_key);
        let (scanner, inviter) = run(sides(&inviter_key, &invite, &invite));
        let (mut scanner, mut inviter) = (scanner.unwrap(), inviter.unwrap());
        assert_eq!(scanner.verification_code(), inviter.verification_code());
        assert_eq!(scanner.verification_code().len(), 7);
        assert_eq!(scanner.remote_static(), inviter_key.public());

        let sealed = scanner.seal(b"hello").unwrap();
        assert_ne!(sealed, b"hello");
        assert_eq!(inviter.open(&sealed).unwrap().as_slice(), b"hello");
        let reply = inviter.seal(b"hi").unwrap();
        assert_eq!(scanner.open(&reply).unwrap().as_slice(), b"hi");
        assert_eq!(
            inviter.open(&sealed),
            Err(PairingError::Handshake),
            "replay rejected"
        );
    }

    #[test]
    fn a_wrong_code_fails_the_handshake() {
        let inviter_key = StaticKey::generate().unwrap();
        let invite = invite_for(&inviter_key);
        let mut guessed = invite.clone();
        guessed.psk[0] ^= 1;
        let (scanner, inviter) = run(sides(&inviter_key, &invite, &guessed));
        assert!(scanner.is_err() || inviter.is_err());
        assert_eq!(inviter.err(), Some(PairingError::Handshake));
    }

    #[test]
    fn an_impostor_inviter_is_detected() {
        let impostor_key = StaticKey::generate().unwrap();
        let real = invite_for(&StaticKey::generate().unwrap());
        // The impostor knows the secret but not the real device's private key.
        let (scanner, _) = run(sides(&impostor_key, &real, &real));
        assert_eq!(scanner.err(), Some(PairingError::UnexpectedDevice));
    }

    #[test]
    fn tampered_messages_are_rejected() {
        let inviter_key = StaticKey::generate().unwrap();
        let invite = invite_for(&inviter_key);
        let (scanner, inviter) = run(sides(&inviter_key, &invite, &invite));
        let (mut scanner, mut inviter) = (scanner.unwrap(), inviter.unwrap());
        let mut sealed = scanner.seal(b"secret").unwrap();
        sealed[0] ^= 1;
        assert_eq!(inviter.open(&sealed), Err(PairingError::Handshake));
    }

    #[test]
    fn oversized_messages_are_refused() {
        let inviter_key = StaticKey::generate().unwrap();
        let invite = invite_for(&inviter_key);
        let (scanner, _) = run(sides(&inviter_key, &invite, &invite));
        let mut scanner = scanner.unwrap();
        assert_eq!(
            scanner.seal(&vec![0; MAX_PLAINTEXT + 1]),
            Err(PairingError::TooLarge)
        );
        assert!(scanner.seal(&vec![0; MAX_PLAINTEXT]).is_ok());
        assert_eq!(
            scanner.open(&vec![0; MAX_MESSAGE + 1]).err(),
            Some(PairingError::TooLarge)
        );
    }

    #[test]
    fn paired_devices_open_a_sync_channel_and_learn_each_others_key() {
        let (a, b) = (
            StaticKey::generate().unwrap(),
            StaticKey::generate().unwrap(),
        );
        let s = Sides {
            scanner: Handshake::sync_initiator(&a).unwrap(),
            inviter: Handshake::sync_responder(&b).unwrap(),
        };
        let (ca, cb) = run(s);
        let (mut ca, mut cb) = (ca.unwrap(), cb.unwrap());
        assert_eq!(
            (ca.remote_static(), cb.remote_static()),
            (b.public(), a.public())
        );
        let sealed = ca.seal(b"sync").unwrap();
        assert_eq!(cb.open(&sealed).unwrap().as_slice(), b"sync");
    }

    #[test]
    fn a_pairing_handshake_cannot_be_used_to_sync() {
        let (a, b) = (
            StaticKey::generate().unwrap(),
            StaticKey::generate().unwrap(),
        );
        let s = Sides {
            scanner: Handshake::sync_initiator(&a).unwrap(),
            inviter: Handshake::inviter(&b, &invite_for(&b)).unwrap(),
        };
        let (ca, cb) = run(s);
        assert!(ca.is_err() || cb.is_err());
    }

    #[test]
    fn a_sync_file_reaches_only_its_recipient_and_names_its_sender() {
        let (sender, recipient, other) = (
            StaticKey::generate().unwrap(),
            StaticKey::generate().unwrap(),
            StaticKey::generate().unwrap(),
        );
        let mut out = Handshake::file_sender(&sender, &recipient.public()).unwrap();
        let first = out.write().unwrap();
        let mut writer = out.finish().unwrap();
        let body = writer.seal(b"items").unwrap();

        let mut inbound = Handshake::file_receiver(&recipient).unwrap();
        inbound.read(&first).unwrap();
        let mut reader = inbound.finish().unwrap();
        assert_eq!(reader.remote_static(), sender.public());
        assert_eq!(reader.open(&body).unwrap().as_slice(), b"items");

        let mut wrong = Handshake::file_receiver(&other).unwrap();
        assert_eq!(wrong.read(&first), Err(PairingError::Handshake));
    }

    #[test]
    fn finishing_early_fails() {
        let key = StaticKey::generate().unwrap();
        let handshake = Handshake::inviter(&key, &invite_for(&key)).unwrap();
        assert!(!handshake.is_finished());
        assert_eq!(handshake.finish().err(), Some(PairingError::Handshake));
    }

    #[test]
    fn keys_round_trip_and_hide_their_secret() {
        let key = StaticKey::generate().unwrap();
        let again = StaticKey::from_parts(*key.private(), key.public());
        assert_eq!(again.public(), key.public());
        assert_eq!(format!("{key:?}"), "StaticKey { .. }");
        assert_ne!(StaticKey::generate().unwrap().public(), key.public());
    }

    #[test]
    fn verification_codes_are_six_grouped_digits() {
        let code = verification_code(b"anything");
        assert!(code.len() == 7 && code.as_bytes()[3] == b' ');
        assert!(code.chars().filter(char::is_ascii_digit).count() == 6);
        assert_ne!(code, verification_code(b"something else"));
        assert_eq!(to_array(&[1, 2]), Err(PairingError::Handshake));
    }

    #[test]
    fn errors_read_well() {
        assert!(
            PairingError::Handshake
                .to_string()
                .contains("wrong or expired")
        );
        assert!(
            PairingError::UnexpectedDevice
                .to_string()
                .contains("unexpected device")
        );
        assert_eq!(PairingError::TooLarge.to_string(), "message too large");
    }
}
