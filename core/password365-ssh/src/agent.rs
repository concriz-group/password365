//! Answering a client: list the keys, sign when the user allows it.

use std::io::{Read, Write};

use crate::keys::AgentKey;
use crate::wire::{Request, Response, read_request, write_response};

/// Where the agent gets keys and permission (the app: its vault and its prompt).
pub trait KeySource {
    /// The usable keys now; empty when the vault is locked or the agent is off.
    fn keys(&self) -> Vec<AgentKey>;
    /// Whether the user lets a program use `key` (asks, or remembers an earlier answer).
    fn approve(&self, key: &AgentKey) -> bool;
}

/// The answer to one request.
pub fn answer(source: &dyn KeySource, request: &Request) -> Response {
    match request {
        Request::Identities => Response::Identities(
            source
                .keys()
                .iter()
                .map(|k| (k.public_blob(), k.title.clone()))
                .collect(),
        ),
        Request::Sign { key, data, .. } => {
            let keys = source.keys();
            let Some(found) = keys.iter().find(|k| &k.public_blob() == key) else {
                return Response::Failure;
            };
            if !source.approve(found) {
                return Response::Failure;
            }
            found
                .sign(data)
                .map_or(Response::Failure, Response::Signature)
        }
        Request::Unsupported(_) => Response::Failure,
    }
}

/// Serves one client connection until it closes or sends something malformed.
pub fn serve<S: Read + Write>(stream: &mut S, source: &dyn KeySource) {
    while let Ok(Some(request)) = read_request(stream) {
        if write_response(stream, &answer(source, &request)).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io::Cursor;

    use super::*;
    use crate::keys::{load_key, tests::ed25519_pem};
    use crate::wire::tests::message;

    struct Fake {
        pem: String,
        allow: bool,
        asked: Cell<u32>,
    }

    impl KeySource for Fake {
        fn keys(&self) -> Vec<AgentKey> {
            vec![load_key("i1", "GitHub", &self.pem, None).unwrap()]
        }
        fn approve(&self, _key: &AgentKey) -> bool {
            self.asked.set(self.asked.get() + 1);
            self.allow
        }
    }

    struct Duplex(Cursor<Vec<u8>>, Vec<u8>);
    impl Read for Duplex {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.0.read(buf)
        }
    }
    impl Write for Duplex {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.1.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn lists_keys_and_signs_only_when_allowed() {
        let fake = Fake {
            pem: ed25519_pem(),
            allow: true,
            asked: Cell::new(0),
        };
        let Response::Identities(keys) = answer(&fake, &Request::Identities) else {
            panic!()
        };
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].1, "GitHub");
        let sign = Request::Sign {
            key: keys[0].0.clone(),
            data: b"x".to_vec(),
            flags: 0,
        };
        assert!(matches!(answer(&fake, &sign), Response::Signature(_)));
        assert_eq!(fake.asked.get(), 1);
        let denied = Fake {
            allow: false,
            ..fake
        };
        assert_eq!(answer(&denied, &sign), Response::Failure);
        let unknown = Request::Sign {
            key: b"other".to_vec(),
            data: vec![],
            flags: 0,
        };
        assert_eq!(answer(&denied, &unknown), Response::Failure);
        assert_eq!(
            answer(&denied, &Request::Unsupported(17)),
            Response::Failure
        );
    }

    #[test]
    fn serves_a_connection_until_it_closes() {
        let fake = Fake {
            pem: ed25519_pem(),
            allow: true,
            asked: Cell::new(0),
        };
        let input = [message(11, &[], None), message(17, &[], None)].concat();
        let mut stream = Duplex(Cursor::new(input), Vec::new());
        serve(&mut stream, &fake);
        assert_eq!(stream.1[4], 12, "identities answer first");
        assert_eq!(
            &stream.1[stream.1.len() - 5..],
            &[0, 0, 0, 1, 5],
            "then failure"
        );
    }
}
