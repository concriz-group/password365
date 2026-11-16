//! Mutual proof that both ends of the local socket belong to this user.
//!
//! The app writes a fresh random key into a file only this user can read
//! (`bridge.key` beside the socket, in the user's private app folder). On
//! every connection, before any request:
//!
//! ```text
//! host → app : MAGIC ‖ host nonce
//! app  → host: app nonce ‖ HMAC(key, "app"  ‖ host nonce ‖ app nonce)
//! host → app : HMAC(key, "host" ‖ app nonce ‖ host nonce)
//! ```
//!
//! A process of another user (on Windows, named pipes are visible to every
//! user, so one could create the pipe first and pose as the app) cannot read
//! the key: the host refuses to talk to it, and the app refuses its requests.

use std::io::{self, Read, Write};
use std::path::Path;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::framing::{read_frame, write_frame};

/// Opens a connection (and names the scheme's version).
const MAGIC: &[u8] = b"P365-BRIDGE-1";
const NONCE: usize = 32;
const TAG: usize = 32;

/// The shared key (32 random bytes).
pub type Key = [u8; 32];

fn refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "the other end did not prove it is Password365 for this user",
    )
}

fn tag(key: &Key, role: &[u8], first: &[u8], second: &[u8]) -> Hmac<Sha256> {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(role);
    mac.update(first);
    mac.update(second);
    mac
}

fn nonce() -> io::Result<[u8; NONCE]> {
    password365_crypto::random::bytes().map_err(|_| io::Error::other("no randomness"))
}

/// A fresh key, written to `path` readable by this user only (replacing any).
pub fn create_key(path: &Path) -> io::Result<Key> {
    let key: Key = nonce()?;
    let _ = std::fs::remove_file(path);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(&key)?;
    Ok(key)
}

/// The key the app wrote.
pub fn read_key(path: &Path) -> io::Result<Key> {
    let bytes = std::fs::read(path)?;
    bytes
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad bridge key"))
}

/// The host's side; `Ok` once the app proved itself (and was answered).
pub fn prove_to_app(stream: &mut (impl Read + Write), key: &Key) -> io::Result<()> {
    let mine = nonce()?;
    write_frame(stream, &[MAGIC, &mine].concat())?;
    let reply = read_frame(stream)?.ok_or_else(refused)?;
    if reply.len() != NONCE + TAG {
        return Err(refused());
    }
    let (theirs, proof) = reply.split_at(NONCE);
    tag(key, b"app", &mine, theirs)
        .verify_slice(proof)
        .map_err(|_| refused())?;
    write_frame(
        stream,
        &tag(key, b"host", theirs, &mine).finalize().into_bytes(),
    )
}

/// The app's side; `Ok` once the host proved itself.
pub fn check_host(stream: &mut (impl Read + Write), key: &Key) -> io::Result<()> {
    let hello = read_frame(stream)?.ok_or_else(refused)?;
    let theirs = hello
        .strip_prefix(MAGIC)
        .filter(|n| n.len() == NONCE)
        .ok_or_else(refused)?;
    let mine = nonce()?;
    let proof = tag(key, b"app", theirs, &mine).finalize().into_bytes();
    write_frame(stream, &[&mine[..], &proof].concat())?;
    let answer = read_frame(stream)?.ok_or_else(refused)?;
    tag(key, b"host", &mine, theirs)
        .verify_slice(&answer)
        .map_err(|_| refused())
}
