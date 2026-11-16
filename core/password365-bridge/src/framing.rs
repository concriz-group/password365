//! Length-prefixed message framing: `u32` little-endian length, then the bytes.
//!
//! This is Chrome's native-messaging wire format (native byte order, which is
//! little-endian on every platform Password365 supports); the local socket uses
//! the same framing so the host can relay messages unchanged.

use std::io::{self, Read, Write};

/// Largest message accepted in either direction (Chrome's limit towards the
/// browser is 1 MiB).
pub const MAX_MESSAGE: usize = 1024 * 1024;

/// Reads one message. Returns `Ok(None)` on a clean end of stream.
pub fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// Writes one message and flushes.
pub fn write_frame(writer: &mut impl Write, body: &[u8]) -> io::Result<()> {
    if body.len() > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "message too large",
        ));
    }
    let len =
        u32::try_from(body.len()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn round_trip_several_messages() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"hello").unwrap();
        write_frame(&mut buf, b"").unwrap();
        let mut r = Cursor::new(buf);
        assert_eq!(read_frame(&mut r).unwrap().unwrap(), b"hello");
        assert_eq!(read_frame(&mut r).unwrap().unwrap(), b"");
        assert_eq!(read_frame(&mut r).unwrap(), None);
    }

    #[test]
    fn length_is_little_endian() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"ab").unwrap();
        assert_eq!(&buf[..4], &[2, 0, 0, 0]);
    }

    #[test]
    fn rejects_oversized_messages() {
        let mut buf = Vec::new();
        assert!(write_frame(&mut buf, &vec![0; MAX_MESSAGE + 1]).is_err());
        let huge = u32::try_from(MAX_MESSAGE + 1).unwrap().to_le_bytes();
        assert_eq!(
            read_frame(&mut Cursor::new(huge)).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn truncated_body_is_an_error() {
        let mut bytes = 10u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(b"abc");
        assert!(read_frame(&mut Cursor::new(bytes)).is_err());
    }

    #[test]
    fn partial_length_counts_as_end_of_stream() {
        assert_eq!(read_frame(&mut Cursor::new(vec![1u8, 0])).unwrap(), None);
    }

    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("boom"))
        }
    }

    #[test]
    fn read_errors_propagate() {
        assert!(read_frame(&mut Broken).is_err());
    }
}
