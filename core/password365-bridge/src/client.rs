//! Host side of the local socket: sends one framed request, reads one response.

use std::io;

use interprocess::local_socket::{Stream, prelude::*};

use crate::endpoint::Endpoint;
use crate::framing::{read_frame, write_frame};

/// A connection to the running desktop app.
pub struct BridgeClient {
    stream: Stream,
}

impl BridgeClient {
    /// Connects; fails if the app is not running, or if what answers cannot
    /// prove it is this user's Password365 ([`crate::auth`]).
    pub fn connect(endpoint: &Endpoint) -> io::Result<Self> {
        let key = crate::auth::read_key(&endpoint.key_path())?;
        let mut stream = Stream::connect(endpoint.name()?)?;
        crate::auth::prove_to_app(&mut stream, &key)?;
        Ok(Self { stream })
    }

    /// Sends a raw JSON request and returns the raw JSON response.
    pub fn exchange(&mut self, request: &[u8]) -> io::Result<Vec<u8>> {
        self.send(request)?;
        self.receive()
    }

    /// Sends one request.
    pub fn send(&mut self, request: &[u8]) -> io::Result<()> {
        write_frame(&mut self.stream, request)
    }

    /// Reads one response.
    pub fn receive(&mut self) -> io::Result<Vec<u8>> {
        read_frame(&mut self.stream)?.ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connecting_without_a_server_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(BridgeClient::connect(&Endpoint::at(dir.path().join("none.sock"))).is_err());
    }
}
