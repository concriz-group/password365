//! Length padding so ciphertext sizes reveal only a coarse size bucket.
//!
//! Layout: `len (u32 BE) || data || zero bytes` up to a multiple of [`BUCKET`].

use zeroize::Zeroizing;

use crate::{Result, VaultError};

/// Padded sizes are multiples of this many bytes.
pub(crate) const BUCKET: usize = 256;
const LEN_PREFIX: usize = 4;

/// Pads `data` to the next bucket boundary.
pub(crate) fn pad(data: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let len = u32::try_from(data.len()).map_err(|_| VaultError::Corrupt("record too large"))?;
    let total = (LEN_PREFIX + data.len()).div_ceil(BUCKET) * BUCKET;
    let mut out = Zeroizing::new(Vec::with_capacity(total));
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(data);
    out.resize(total, 0);
    Ok(out)
}

/// Removes padding added by [`pad`].
pub(crate) fn unpad(padded: &[u8]) -> Result<&[u8]> {
    let prefix: [u8; LEN_PREFIX] = padded
        .get(..LEN_PREFIX)
        .and_then(|p| p.try_into().ok())
        .ok_or(VaultError::Corrupt("padding prefix missing"))?;
    let len = u32::from_be_bytes(prefix) as usize;
    padded
        .get(LEN_PREFIX..LEN_PREFIX + len)
        .ok_or(VaultError::Corrupt("padding length out of range"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_to_bucket_multiple() {
        assert_eq!(pad(&[]).unwrap().len(), BUCKET);
        assert_eq!(pad(&[1; BUCKET - LEN_PREFIX]).unwrap().len(), BUCKET);
        assert_eq!(
            pad(&[1; BUCKET - LEN_PREFIX + 1]).unwrap().len(),
            2 * BUCKET
        );
    }

    #[test]
    fn round_trip() {
        for len in [0, 1, 100, 252, 253, 1000] {
            let data = vec![0xAB; len];
            assert_eq!(unpad(&pad(&data).unwrap()).unwrap(), data.as_slice());
        }
    }

    #[test]
    fn unpad_rejects_short_input() {
        assert!(unpad(&[0, 0]).is_err());
    }

    #[test]
    fn unpad_rejects_length_beyond_buffer() {
        let mut padded = pad(b"abc").unwrap().to_vec();
        padded[..4].copy_from_slice(&10_000u32.to_be_bytes());
        assert!(unpad(&padded).is_err());
    }
}
