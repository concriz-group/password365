//! Device identifiers.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Random, stable identifier of one install (never derived from hardware).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(Uuid);

impl DeviceId {
    /// Generates a new random device id.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Builds an id from its 16 raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    /// The 16 raw bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::str::FromStr for DeviceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ids_are_unique_and_round_trip() {
        let a = DeviceId::new();
        assert_ne!(a, DeviceId::default());
        assert_eq!(DeviceId::from_bytes(*a.as_bytes()), a);
        assert_eq!(a.to_string().len(), 36);
        assert_eq!(a.to_string().parse::<DeviceId>().unwrap(), a);
        assert!("nope".parse::<DeviceId>().is_err());
    }
}
