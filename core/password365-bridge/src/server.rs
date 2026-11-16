//! Desktop-app side of the local socket.

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use interprocess::local_socket::{ListenerOptions, Stream, prelude::*};

use crate::endpoint::Endpoint;
use crate::framing::{read_frame, write_frame};

/// Turns one raw JSON request into a raw JSON response.
pub type Handler = Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

/// Starts listening on `endpoint`; each connection is served on its own thread.
///
/// On Unix only processes running as the same user are accepted (checked with
/// the socket's peer credentials, on top of the 0700 directory).
pub fn serve(endpoint: &Endpoint, handler: Handler) -> io::Result<JoinHandle<()>> {
    endpoint.prepare()?;
    let listener = ListenerOptions::new()
        .name(endpoint.name()?)
        .try_overwrite(true)
        .create_sync()?;
    #[cfg(unix)]
    let own_uid = endpoint.owner_uid()?;
    Ok(thread::spawn(move || {
        for connection in listener.incoming().filter_map(Result::ok) {
            #[cfg(unix)]
            if !same_user(&connection, own_uid) {
                continue;
            }
            let handler = Arc::clone(&handler);
            thread::spawn(move || {
                let _ = serve_connection(connection, handler.as_ref());
            });
        }
    }))
}

#[cfg(unix)]
fn same_user(connection: &Stream, own_uid: u32) -> bool {
    connection.peer_creds().ok().and_then(|c| c.euid()) == Some(own_uid)
}

/// Answers framed requests until the peer disconnects.
pub fn serve_connection(
    mut stream: impl Read + Write,
    handler: &(dyn Fn(&[u8]) -> Vec<u8> + Send + Sync),
) -> io::Result<()> {
    while let Some(request) = read_frame(&mut stream)? {
        write_frame(&mut stream, &handler(&request))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::client::BridgeClient;

    /// In-memory duplex: reads from `input`, writes into `output`.
    struct Duplex {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }
    impl Read for Duplex {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.input.read(buf)
        }
    }
    impl Write for Duplex {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.output.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn echo_upper() -> Handler {
        Arc::new(|req: &[u8]| req.to_ascii_uppercase())
    }

    #[test]
    fn serves_every_frame_until_eof() {
        let mut input = Vec::new();
        write_frame(&mut input, b"a").unwrap();
        write_frame(&mut input, b"bc").unwrap();
        let mut duplex = Duplex {
            input: Cursor::new(input),
            output: Vec::new(),
        };
        serve_connection(&mut duplex, echo_upper().as_ref()).unwrap();
        let mut out = Cursor::new(duplex.output);
        assert_eq!(read_frame(&mut out).unwrap().unwrap(), b"A");
        assert_eq!(read_frame(&mut out).unwrap().unwrap(), b"BC");
    }

    #[cfg(unix)]
    #[test]
    fn end_to_end_over_a_real_socket() {
        let dir = tempfile::tempdir().unwrap();
        let endpoint = Endpoint::at(dir.path().join("b.sock"));
        serve(&endpoint, echo_upper()).unwrap();
        let mut client = BridgeClient::connect(&endpoint).unwrap();
        assert_eq!(client.exchange(b"hello").unwrap(), b"HELLO");
        assert_eq!(client.exchange(b"again").unwrap(), b"AGAIN");
        // A second, concurrent client is served too.
        let mut other = BridgeClient::connect(&endpoint).unwrap();
        assert_eq!(other.exchange(b"x").unwrap(), b"X");
    }

    #[cfg(unix)]
    #[test]
    fn restarting_reclaims_a_stale_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.sock");
        std::fs::write(&path, b"stale").unwrap();
        let endpoint = Endpoint::at(path);
        serve(&endpoint, echo_upper()).unwrap();
        assert_eq!(
            BridgeClient::connect(&endpoint)
                .unwrap()
                .exchange(b"ok")
                .unwrap(),
            b"OK"
        );
    }
}
