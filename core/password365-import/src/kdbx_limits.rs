//! Checks a `KeePass` file's key-derivation settings before deriving anything:
//! a hostile file could ask for terabytes of memory or billions of rounds and
//! hang or kill the app. Read from the unencrypted outer header.

use crate::{ImportError, Result};

/// Most Argon2 memory accepted (`KeePassXC`'s largest preset is 1 GiB).
pub const MAX_ARGON2_MEMORY: u64 = 1024 * 1024 * 1024;
/// Most Argon2 passes accepted.
pub const MAX_ARGON2_ITERATIONS: u64 = 100;
/// Most Argon2 lanes accepted.
pub const MAX_ARGON2_PARALLELISM: u64 = 64;
/// Most AES-KDF rounds accepted (a few seconds on a current computer).
pub const MAX_AES_ROUNDS: u64 = 300_000_000;

const KDBX: [u8; 8] = [0x03, 0xd9, 0xa2, 0x9a, 0x67, 0xfb, 0x4b, 0xb5];
const KDB: [u8; 8] = [0x03, 0xd9, 0xa2, 0x9a, 0x65, 0xfb, 0x4b, 0xb5];

fn too_costly() -> ImportError {
    ImportError::Format(
        "this KeePass database asks for an unreasonable amount of work to open".into(),
    )
}

fn le_u64(bytes: &[u8]) -> Option<u64> {
    match bytes.len() {
        4 => Some(u64::from(u32::from_le_bytes(bytes.try_into().ok()?))),
        8 => Some(u64::from_le_bytes(bytes.try_into().ok()?)),
        _ => None,
    }
}

/// Fails if the file's key derivation is beyond the limits above. Files
/// this does not recognise are left to the parser.
pub fn check(data: &[u8]) -> Result<()> {
    if data.len() >= 124 && data[..8] == KDB {
        // KeePass 1: transform rounds at a fixed place.
        let rounds = le_u64(&data[120..124]).unwrap_or(0);
        return if rounds > MAX_AES_ROUNDS {
            Err(too_costly())
        } else {
            Ok(())
        };
    }
    if data.len() < 12 || data[..8] != KDBX {
        return Ok(());
    }
    let major = u16::from_le_bytes([data[10], data[11]]);
    let mut at = 12;
    loop {
        let Some(&id) = data.get(at) else {
            return Ok(());
        };
        let (size, header) = if major >= 4 {
            let size = data.get(at + 1..at + 5).and_then(le_u64).unwrap_or(0);
            (usize::try_from(size).unwrap_or(usize::MAX), 5)
        } else {
            let size = data
                .get(at + 1..at + 3)
                .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
            (usize::from(size), 3)
        };
        let Some(value) = data.get(at + header..(at + header).saturating_add(size)) else {
            return Ok(());
        };
        match id {
            0 => return Ok(()),
            // KDBX 3: AES-KDF rounds.
            6 if major < 4 => {
                if le_u64(value).unwrap_or(0) > MAX_AES_ROUNDS {
                    return Err(too_costly());
                }
            }
            // KDBX 4: the KDF's parameters (a variant dictionary).
            11 if major >= 4 => check_parameters(value)?,
            _ => {}
        }
        at += header + size;
    }
}

/// `R` (AES rounds), `M` (Argon2 memory, bytes), `I` (passes), `P` (lanes).
fn check_parameters(dict: &[u8]) -> Result<()> {
    let mut at = 2; // version
    while let Some(&kind) = dict.get(at) {
        if kind == 0 {
            break;
        }
        let read_len = |from: usize| {
            dict.get(from..from + 4)
                .and_then(le_u64)
                .and_then(|n| usize::try_from(n).ok())
        };
        let Some(key_len) = read_len(at + 1) else {
            break;
        };
        let key = dict.get(at + 5..at + 5 + key_len).unwrap_or_default();
        let value_at = at + 5 + key_len;
        let Some(value_len) = read_len(value_at) else {
            break;
        };
        let value = dict
            .get(value_at + 4..value_at + 4 + value_len)
            .unwrap_or_default();
        let number = le_u64(value).unwrap_or(0);
        let limit = match key {
            b"R" => MAX_AES_ROUNDS,
            b"M" => MAX_ARGON2_MEMORY,
            b"I" => MAX_ARGON2_ITERATIONS,
            b"P" => MAX_ARGON2_PARALLELISM,
            _ => u64::MAX,
        };
        if number > limit {
            return Err(too_costly());
        }
        at = value_at + 4 + value_len;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: u8, key: &[u8], value: &[u8]) -> Vec<u8> {
        let mut out = vec![kind];
        out.extend(u32::try_from(key.len()).unwrap().to_le_bytes());
        out.extend(key);
        out.extend(u32::try_from(value.len()).unwrap().to_le_bytes());
        out.extend(value);
        out
    }

    fn kdbx4(params: &[Vec<u8>]) -> Vec<u8> {
        let mut dict = vec![0, 1];
        for p in params {
            dict.extend(p);
        }
        dict.push(0);
        let mut out = KDBX.to_vec();
        out.extend([1, 0, 4, 0]);
        out.push(11);
        out.extend(u32::try_from(dict.len()).unwrap().to_le_bytes());
        out.extend(dict);
        out.extend([0, 0, 0, 0, 0]);
        out
    }

    #[test]
    fn reasonable_argon2_settings_pass_and_absurd_ones_do_not() {
        let ok = kdbx4(&[
            entry(0x05, b"M", &(64u64 << 20).to_le_bytes()),
            entry(0x05, b"I", &10u64.to_le_bytes()),
            entry(0x04, b"P", &2u32.to_le_bytes()),
        ]);
        assert!(check(&ok).is_ok());
        for bad in [
            entry(0x05, b"M", &(1u64 << 40).to_le_bytes()),
            entry(0x05, b"I", &1_000_000u64.to_le_bytes()),
            entry(0x04, b"P", &1000u32.to_le_bytes()),
            entry(0x05, b"R", &u64::MAX.to_le_bytes()),
        ] {
            assert!(check(&kdbx4(&[bad])).is_err());
        }
    }

    #[test]
    fn kdbx3_and_keepass1_rounds_are_checked() {
        let mut v3 = KDBX.to_vec();
        v3.extend([1, 0, 3, 0, 6, 8, 0]);
        v3.extend(u64::MAX.to_le_bytes());
        v3.extend([0, 0, 0]);
        assert!(check(&v3).is_err());
        let mut v1 = KDB.to_vec();
        v1.resize(120, 0);
        v1.extend(u32::MAX.to_le_bytes());
        assert!(check(&v1).is_err());
    }

    #[test]
    fn other_files_and_cut_headers_are_left_to_the_parser() {
        assert!(check(b"not a database").is_ok());
        assert!(check(&KDBX).is_ok());
        let mut cut = kdbx4(&[entry(0x05, b"M", &(1u64 << 40).to_le_bytes())]);
        cut.truncate(20);
        assert!(check(&cut).is_ok());
    }
}
