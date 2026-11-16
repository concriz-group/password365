//! Relays framed messages from the browser (stdin) to the app and back (stdout).

use std::io::{self, Read, Write};

use password365_bridge::framing::{read_frame, write_frame};
use password365_bridge::protocol::Response;

/// Something that can answer one raw request (the app connection).
pub trait Exchange {
    /// Sends a request and returns the response.
    fn exchange(&mut self, request: &[u8]) -> io::Result<Vec<u8>>;
}

impl Exchange for password365_bridge::client::BridgeClient {
    fn exchange(&mut self, request: &[u8]) -> io::Result<Vec<u8>> {
        password365_bridge::client::BridgeClient::exchange(self, request)
    }
}

/// Code returned when the desktop app is not running / not reachable.
pub const APP_NOT_RUNNING: &str = "app_not_running";

/// Relays until the browser closes stdin. Connects lazily and reconnects once
/// per message, so starting the app after the browser works.
pub fn relay<C: Exchange>(
    input: &mut impl Read,
    output: &mut impl Write,
    mut connect: impl FnMut() -> io::Result<C>,
) -> io::Result<()> {
    let mut connection: Option<C> = None;
    while let Some(request) = read_frame(input)? {
        let response = forward(&mut connection, &mut connect, &request)
            .unwrap_or_else(|| not_running(&request));
        write_frame(output, &response)?;
    }
    Ok(())
}

/// Tries the existing connection, then one fresh connection.
fn forward<C: Exchange>(
    connection: &mut Option<C>,
    connect: &mut impl FnMut() -> io::Result<C>,
    request: &[u8],
) -> Option<Vec<u8>> {
    for _ in 0..2 {
        if connection.is_none() {
            *connection = connect().ok();
        }
        let conn = connection.as_mut()?;
        match conn.exchange(request) {
            Ok(response) => return Some(response),
            Err(_) => *connection = None,
        }
    }
    None
}

fn not_running(request: &[u8]) -> Vec<u8> {
    let id = serde_json::from_slice::<serde_json::Value>(request)
        .ok()
        .and_then(|v| v.get("id").and_then(serde_json::Value::as_u64))
        .unwrap_or(0);
    let response = Response::failure(
        id,
        APP_NOT_RUNNING,
        "Password365 is not running — open the app and unlock it",
    );
    serde_json::to_vec(&response).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    struct Echo;
    impl Exchange for Echo {
        fn exchange(&mut self, request: &[u8]) -> io::Result<Vec<u8>> {
            Ok([b"re:", request].concat())
        }
    }

    struct FailsOnce(bool);
    impl Exchange for FailsOnce {
        fn exchange(&mut self, request: &[u8]) -> io::Result<Vec<u8>> {
            if std::mem::replace(&mut self.0, false) {
                return Err(io::Error::other("broken pipe"));
            }
            Ok(request.to_vec())
        }
    }

    fn frames(messages: &[&[u8]]) -> Cursor<Vec<u8>> {
        let mut buf = Vec::new();
        for m in messages {
            write_frame(&mut buf, m).unwrap();
        }
        Cursor::new(buf)
    }

    fn read_all(bytes: Vec<u8>) -> Vec<Vec<u8>> {
        let mut c = Cursor::new(bytes);
        std::iter::from_fn(|| read_frame(&mut c).unwrap()).collect()
    }

    #[test]
    fn relays_every_message_over_one_connection() {
        let mut connects = 0;
        let mut out = Vec::new();
        relay(&mut frames(&[b"a", b"b"]), &mut out, || {
            connects += 1;
            Ok(Echo)
        })
        .unwrap();
        assert_eq!(read_all(out), vec![b"re:a".to_vec(), b"re:b".to_vec()]);
        assert_eq!(connects, 1);
    }

    #[test]
    fn reconnects_after_a_broken_connection() {
        let mut out = Vec::new();
        let mut first = true;
        relay(&mut frames(&[b"x"]), &mut out, || {
            let fail = std::mem::replace(&mut first, false);
            Ok(FailsOnce(fail))
        })
        .unwrap();
        assert_eq!(read_all(out), vec![b"x".to_vec()]);
    }

    #[test]
    fn answers_app_not_running_with_the_request_id() {
        let mut out = Vec::new();
        relay::<Echo>(
            &mut frames(&[br#"{"id":42,"type":"status"}"#, b"garbage"]),
            &mut out,
            || Err(io::Error::from(io::ErrorKind::NotFound)),
        )
        .unwrap();
        let responses: Vec<serde_json::Value> = read_all(out)
            .iter()
            .map(|r| serde_json::from_slice(r).unwrap())
            .collect();
        assert_eq!(responses[0]["id"], 42);
        assert_eq!(responses[0]["error"]["code"], APP_NOT_RUNNING);
        assert_eq!(responses[1]["id"], 0);
    }
}
