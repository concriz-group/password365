//! Compact binary (postcard) encoding into zeroizing buffers.

use serde::Serialize;
use serde::de::DeserializeOwned;
use zeroize::Zeroizing;

use crate::{Result, VaultError};

/// Serializes `value`; the buffer is wiped when dropped.
pub(crate) fn encode<T: Serialize>(value: &T) -> Result<Zeroizing<Vec<u8>>> {
    postcard::to_allocvec(value)
        .map(Zeroizing::new)
        .map_err(|_| VaultError::Corrupt("cannot encode value"))
}

/// Deserializes a value produced by [`encode`].
pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    postcard::from_bytes(bytes).map_err(|_| VaultError::Corrupt("cannot decode value"))
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        a: u32,
        b: String,
    }

    #[test]
    fn round_trip() {
        let value = Sample {
            a: 7,
            b: "x".into(),
        };
        let bytes = encode(&value).unwrap();
        assert_eq!(decode::<Sample>(&bytes).unwrap(), value);
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(matches!(
            decode::<Sample>(&[0xFF]),
            Err(VaultError::Corrupt(_))
        ));
    }
}
