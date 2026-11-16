//! Syncing over TCP on the local network.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use password365_sync::pairing::{Handshake, StaticKey};
use password365_vault::Session;

use crate::Result;
use crate::secure::SecureStream;
use crate::sync::{self, SyncOutcome};

/// How long to wait for the other device before giving up.
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// Port each desktop listens on for syncs from its paired devices.
pub const SYNC_PORT: u16 = 47_480;

/// Opens an encrypted connection to a device at `addr` (who it is gets
/// checked against the paired devices once the sync starts).
pub fn connect(addr: SocketAddr, key: &StaticKey) -> Result<SecureStream<TcpStream>> {
    let stream = TcpStream::connect_timeout(&addr, TIMEOUT)?;
    prepare(&stream)?;
    SecureStream::establish(stream, Handshake::sync_initiator(key)?)
}

/// Accepts one encrypted connection on `listener`; returns it with the
/// caller's address.
pub fn accept(
    listener: &TcpListener,
    key: &StaticKey,
) -> Result<(SecureStream<TcpStream>, SocketAddr)> {
    let (stream, from) = listener.accept()?;
    Ok((secure_incoming(stream, key)?, from))
}

/// Runs the responder handshake on an already-accepted connection.
pub fn secure_incoming(stream: TcpStream, key: &StaticKey) -> Result<SecureStream<TcpStream>> {
    prepare(&stream)?;
    SecureStream::establish(stream, Handshake::sync_responder(key)?)
}

/// Connects to a paired device at `addr` and syncs.
pub fn sync_to(addr: SocketAddr, session: &mut Session) -> Result<SyncOutcome> {
    let mut secure = connect(addr, &session.static_key())?;
    sync::sync(&mut secure, session)
}

/// Accepts one incoming sync from a paired device on `listener`.
pub fn accept_sync(listener: &TcpListener, session: &mut Session) -> Result<SyncOutcome> {
    let (mut secure, _) = accept(listener, &session.static_key())?;
    sync::sync(&mut secure, session)
}

fn prepare(stream: &TcpStream) -> Result<()> {
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    stream.set_nodelay(true)?;
    Ok(())
}
