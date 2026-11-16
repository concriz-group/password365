//! Agent messages on the wire: `uint32 length, byte type, contents`.

use std::io::{self, Read, Write};

/// Longest message accepted (the protocol suggests 256 KiB).
pub const MAX_MESSAGE: usize = 256 * 1024;

const FAILURE: u8 = 5;
const REQUEST_IDENTITIES: u8 = 11;
const IDENTITIES_ANSWER: u8 = 12;
const SIGN_REQUEST: u8 = 13;
const SIGN_RESPONSE: u8 = 14;

/// What a client asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// List the keys.
    Identities,
    /// Sign `data` with the key whose public blob is `key`.
    Sign {
        /// Public key blob.
        key: Vec<u8>,
        /// Bytes to sign.
        data: Vec<u8>,
        /// Signature flags (RSA hash choice; ignored for Ed25519 / ECDSA).
        flags: u32,
    },
    /// Anything else (adding keys, locking …): refused.
    Unsupported(u8),
}

/// What the agent answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// The keys: public blob and comment.
    Identities(Vec<(Vec<u8>, String)>),
    /// A signature blob.
    Signature(Vec<u8>),
    /// Refused or failed.
    Failure,
}

fn u32_at(bytes: &[u8], at: &mut usize) -> io::Result<u32> {
    let end = at
        .checked_add(4)
        .filter(|e| *e <= bytes.len())
        .ok_or_else(short)?;
    let value = u32::from_be_bytes(bytes[*at..end].try_into().map_err(|_| short())?);
    *at = end;
    Ok(value)
}

fn string_at(bytes: &[u8], at: &mut usize) -> io::Result<Vec<u8>> {
    let len = u32_at(bytes, at)? as usize;
    let end = at
        .checked_add(len)
        .filter(|e| *e <= bytes.len())
        .ok_or_else(short)?;
    let value = bytes[*at..end].to_vec();
    *at = end;
    Ok(value)
}

fn short() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "truncated agent message")
}

/// Reads one request; `Ok(None)` when the client closed the connection.
pub fn read_request<R: Read>(reader: &mut R) -> io::Result<Option<Request>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len) {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        other => other?,
    }
    let len = u32::from_be_bytes(len) as usize;
    if len == 0 || len > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad agent message length",
        ));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    let mut at = 1;
    Ok(Some(match body[0] {
        REQUEST_IDENTITIES => Request::Identities,
        SIGN_REQUEST => {
            let key = string_at(&body, &mut at)?;
            let data = string_at(&body, &mut at)?;
            let flags = u32_at(&body, &mut at)?;
            Request::Sign { key, data, flags }
        }
        other => Request::Unsupported(other),
    }))
}

fn put_string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Writes one response.
pub fn write_response<W: Write>(writer: &mut W, response: &Response) -> io::Result<()> {
    let mut body = Vec::new();
    match response {
        Response::Failure => body.push(FAILURE),
        Response::Signature(blob) => {
            body.push(SIGN_RESPONSE);
            put_string(&mut body, blob);
        }
        Response::Identities(keys) => {
            body.push(IDENTITIES_ANSWER);
            body.extend_from_slice(&u32::try_from(keys.len()).unwrap_or(u32::MAX).to_be_bytes());
            for (blob, comment) in keys {
                put_string(&mut body, blob);
                put_string(&mut body, comment.as_bytes());
            }
        }
    }
    let len = u32::try_from(body.len()).map_err(|_| io::Error::other("response too large"))?;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn message(kind: u8, parts: &[&[u8]], flags: Option<u32>) -> Vec<u8> {
        let mut body = vec![kind];
        for p in parts {
            put_string(&mut body, p);
        }
        if let Some(f) = flags {
            body.extend_from_slice(&f.to_be_bytes());
        }
        let mut out = u32::try_from(body.len()).unwrap().to_be_bytes().to_vec();
        out.extend(body);
        out
    }

    #[test]
    fn reads_requests() {
        let mut input = [
            message(11, &[], None),
            message(13, &[b"key", b"data"], Some(2)),
            message(17, &[], None),
        ]
        .concat();
        let mut reader = input.as_slice();
        assert_eq!(
            read_request(&mut reader).unwrap(),
            Some(Request::Identities)
        );
        assert_eq!(
            read_request(&mut reader).unwrap(),
            Some(Request::Sign {
                key: b"key".to_vec(),
                data: b"data".to_vec(),
                flags: 2
            })
        );
        assert_eq!(
            read_request(&mut reader).unwrap(),
            Some(Request::Unsupported(17))
        );
        assert_eq!(read_request(&mut reader).unwrap(), None);
        input = message(13, &[b"key"], None);
        assert!(read_request(&mut input.as_slice()).is_err(), "truncated");
        assert!(read_request(&mut [0, 0, 0, 0].as_slice()).is_err(), "empty");
        assert!(
            read_request(&mut [0xff, 0xff, 0xff, 0xff].as_slice()).is_err(),
            "too long"
        );
    }

    #[test]
    fn writes_responses() {
        let mut out = Vec::new();
        write_response(&mut out, &Response::Failure).unwrap();
        assert_eq!(out, [0, 0, 0, 1, 5]);
        out.clear();
        write_response(
            &mut out,
            &Response::Identities(vec![(b"k".to_vec(), "c".into())]),
        )
        .unwrap();
        assert_eq!(
            out,
            [
                0, 0, 0, 15, 12, 0, 0, 0, 1, 0, 0, 0, 1, b'k', 0, 0, 0, 1, b'c'
            ]
        );
        out.clear();
        write_response(&mut out, &Response::Signature(b"s".to_vec())).unwrap();
        assert_eq!(out, [0, 0, 0, 6, 14, 0, 0, 0, 1, b's']);
    }
}
