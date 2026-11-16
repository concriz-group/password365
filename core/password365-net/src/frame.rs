//! Length-prefixed frames: a 2-byte big-endian length, then that many bytes.
//! A Noise message never exceeds 65,535 bytes, so two bytes always suffice.

use std::io::{self, Read, Write};

/// Largest frame body.
pub const MAX_FRAME: usize = u16::MAX as usize;

/// Writes one frame.
pub fn write(out: &mut impl Write, body: &[u8]) -> io::Result<()> {
    let len = u16::try_from(body.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too large"))?;
    out.write_all(&len.to_be_bytes())?;
    out.write_all(body)?;
    out.flush()
}

/// Reads one frame. An empty frame is a protocol error (never sent).
pub fn read(input: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len = [0u8; 2];
    input.read_exact(&mut len)?;
    let len = usize::from(u16::from_be_bytes(len));
    if len == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty frame"));
    }
    let mut body = vec![0u8; len];
    input.read_exact(&mut body)?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn frames_round_trip_back_to_back() {
        let mut wire = Vec::new();
        write(&mut wire, b"one").unwrap();
        write(&mut wire, &vec![7; MAX_FRAME]).unwrap();
        let mut input = Cursor::new(wire);
        assert_eq!(read(&mut input).unwrap(), b"one");
        assert_eq!(read(&mut input).unwrap().len(), MAX_FRAME);
        assert_eq!(
            read(&mut input).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn oversized_empty_and_truncated_frames_fail() {
        let mut sink = Vec::new();
        assert_eq!(
            write(&mut sink, &vec![0; MAX_FRAME + 1])
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            read(&mut Cursor::new(vec![0, 0])).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            read(&mut Cursor::new(vec![0, 5, 1])).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
