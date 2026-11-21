//! Pairing a new device in person: QR invite, Noise handshake, verification code.

pub mod invite;
pub mod noise;

pub use invite::{Invite, InviteError, LINK_PREFIX, link_for};
pub use noise::{Channel, Handshake, PairingError, StaticKey};
