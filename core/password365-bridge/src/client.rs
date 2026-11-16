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
    /// Connects; fails if the app is not running.
    pub fn connect(endpoint: &Endpoint) -> io::Result<Self> {
        Ok(Self {
            stream: Stream::connect(endpoint.name()?)?,
        })
    }

    /// Sends a raw JSON request and returns the raw JSON response.
    pub fn exchange(&mut self, request: &[u8]) -> io::Result<Vec<u8>> {
        write_frame(&mut self.stream, request)?;
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
