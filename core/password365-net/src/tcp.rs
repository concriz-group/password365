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

/// Longest an incoming connection may take to finish the handshake: a
/// stranger trickling bytes must not hold a connection for long.
pub const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(10);

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

/// Runs the responder handshake on an already-accepted connection; closes
/// the connection if it is not done within [`HANDSHAKE_DEADLINE`].
pub fn secure_incoming(stream: TcpStream, key: &StaticKey) -> Result<SecureStream<TcpStream>> {
    secure_incoming_within(stream, key, HANDSHAKE_DEADLINE)
}

fn secure_incoming_within(
    stream: TcpStream,
    key: &StaticKey,
    deadline: Duration,
) -> Result<SecureStream<TcpStream>> {
    prepare(&stream)?;
    let watched = stream.try_clone()?;
    let (done, finished) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        if finished.recv_timeout(deadline) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
            let _ = watched.shutdown(std::net::Shutdown::Both);
        }
    });
    let secure = SecureStream::establish(stream, Handshake::sync_responder(key)?);
    let _ = done.send(());
    secure
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

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::time::Instant;

    use super::*;

    #[test]
    fn a_stranger_trickling_bytes_is_cut_off() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let slow = std::thread::spawn(move || {
            let mut s = TcpStream::connect(addr).unwrap();
            for _ in 0..40 {
                if s.write_all(&[0]).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let (stream, _) = listener.accept().unwrap();
        let key = StaticKey::generate().unwrap();
        let started = Instant::now();
        assert!(secure_incoming_within(stream, &key, Duration::from_millis(300)).is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        slow.join().unwrap();
    }
}
