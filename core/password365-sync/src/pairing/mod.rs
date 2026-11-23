//! Pairing a new device in person: QR invite, Noise handshake, verification code.

pub mod invite;
pub mod noise;

pub use invite::{Invite, InviteError, JOIN_LINK_PREFIX, LINK_PREFIX, join_link_for, link_for};
pub use noise::{Channel, Handshake, PairingError, StaticKey};
