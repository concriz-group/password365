//! Reaching a paired device whose network address may have changed.
//!
//! The last known address is tried first; if nothing answers there (or a
//! different device does), the addresses found on the local network are
//! tried, and one is used only when the handshake proves it is that device
//! (its key matches the one pinned at pairing). Discovery is the caller's:
//! mDNS on computers, the system's Bonjour on iPhone.

use std::net::{IpAddr, SocketAddr, TcpStream};

use password365_net::secure::SecureStream;
use password365_net::tcp;
use password365_sync::pairing::StaticKey;

use crate::devices::net_error;
use crate::{AppError, Result};

/// A connection to the expected device, and its new IP when it was found at
/// a different address than the known one (to remember for next time).
pub struct Reached {
    /// The secured connection, already proven to be the expected device.
    pub stream: SecureStream<TcpStream>,
    /// Its new IP, when found away from the known address.
    pub moved_to: Option<IpAddr>,
}

/// Connects to the device whose pinned key is `expected`. `discover` runs
/// only if the known address does not lead to it.
pub fn reach(
    known: Option<SocketAddr>,
    key: &StaticKey,
    expected: &[u8; 32],
    discover: impl FnOnce() -> Vec<SocketAddr>,
) -> Result<Reached> {
    let mut last_error = None;
    if let Some(addr) = known {
        match tcp::connect(addr, key) {
            Ok(stream) if stream.remote_static() == *expected => {
                return Ok(Reached {
                    stream,
                    moved_to: None,
                });
            }
            Ok(_) => last_error = Some(another_device()),
            Err(e) => last_error = Some(net_error(e)),
        }
    }
    for addr in discover() {
        if Some(addr) == known {
            continue;
        }
        if let Ok(stream) = tcp::connect(addr, key)
            && stream.remote_static() == *expected
        {
            return Ok(Reached {
                stream,
                moved_to: Some(addr.ip()),
            });
        }
    }
    Err(last_error
        .unwrap_or_else(|| AppError::Io("The device was not found on this network.".into())))
}

fn another_device() -> AppError {
    AppError::Io("Another device answered at that address.".into())
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, TcpListener};
    use std::thread;

    use super::*;

    fn key() -> StaticKey {
        StaticKey::generate().unwrap()
    }

    /// A device listening on a fresh local port; answers one handshake.
    fn device(k: StaticKey) -> (SocketAddr, thread::JoinHandle<bool>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            tcp::secure_incoming(stream, &k).is_ok()
        });
        (addr, handle)
    }

    fn dead() -> SocketAddr {
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
    }

    #[test]
    fn the_known_address_is_used_without_looking_further() {
        let (me, them) = (key(), key());
        let expected = them.public();
        let (addr, server) = device(them);
        let reached = reach(Some(addr), &me, &expected, || panic!("no discovery needed")).unwrap();
        assert!(reached.moved_to.is_none());
        assert!(server.join().unwrap());
    }

    #[test]
    fn a_device_that_moved_is_found_and_its_new_address_returned() {
        let (me, them) = (key(), key());
        let expected = them.public();
        let (addr, server) = device(them);
        let reached = reach(Some(dead()), &me, &expected, || vec![dead(), addr]).unwrap();
        assert_eq!(reached.moved_to, Some(addr.ip()));
        assert_eq!(reached.stream.remote_static(), expected);
        assert!(server.join().unwrap());
    }

    #[test]
    fn a_device_never_reached_before_is_found_too() {
        let (me, them) = (key(), key());
        let expected = them.public();
        let (addr, server) = device(them);
        assert!(
            reach(None, &me, &expected, || vec![addr])
                .unwrap()
                .moved_to
                .is_some()
        );
        server.join().unwrap();
    }

    #[test]
    fn a_stranger_at_a_discovered_address_is_refused() {
        let (me, them, stranger) = (key(), key(), key());
        let (addr, server) = device(stranger);
        let err = reach(Some(dead()), &me, &them.public(), || vec![addr])
            .err()
            .unwrap();
        assert_eq!(err.code(), "io");
        server.join().unwrap();
    }

    #[test]
    fn a_stranger_at_the_known_address_leads_to_discovery() {
        let (me, them, stranger) = (key(), key(), key());
        let expected = them.public();
        let (old, s1) = device(stranger);
        let (new, s2) = device(them);
        let reached = reach(Some(old), &me, &expected, || vec![new]).unwrap();
        assert_eq!(reached.moved_to, Some(new.ip()));
        s1.join().unwrap();
        s2.join().unwrap();
    }

    #[test]
    fn nothing_found_is_reported() {
        let (me, them) = (key(), key());
        let err = reach(Some(dead()), &me, &them.public(), Vec::new)
            .err()
            .unwrap();
        assert_eq!(err.code(), "io");
        let err = reach(None, &me, &them.public(), Vec::new).err().unwrap();
        assert!(err.to_string().contains("not found"), "{err}");
    }
}
