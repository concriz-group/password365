//! Device-to-device transport for Password365: length-prefixed frames carrying a
//! Noise channel, over TCP on the local network. No server is involved.

pub mod discovery;
mod error;
pub mod frame;
pub mod pairing;
pub mod secure;
pub mod sync;
pub mod sync_file;
pub mod tcp;

pub use error::{NetError, Result};
