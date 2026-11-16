//! Strongly typed identifiers (random UUID v4).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ModelError;

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generates a new random identifier.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Builds an identifier from its 16 raw bytes.
            #[must_use]
            pub fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(Uuid::from_bytes(bytes))
            }

            /// The 16 raw bytes (used for associated data).
            #[must_use]
            pub fn as_bytes(&self) -> &[u8; 16] {
                self.0.as_bytes()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = ModelError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s.trim()).map(Self).map_err(|_| ModelError::InvalidId)
            }
        }
    };
}

define_id!(
    /// Identifier of a file attached to an item.
    AttachmentId
);

define_id!(
    /// Identifies a vault (Personal, Work, …).
    VaultId
);
define_id!(
    /// Identifies an item inside a vault.
    ItemId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ids_are_unique() {
        assert_ne!(VaultId::new(), VaultId::new());
        assert_ne!(ItemId::default(), ItemId::default());
    }

    #[test]
    fn display_and_parse_round_trip() {
        let id = ItemId::new();
        assert_eq!(id.to_string().parse::<ItemId>().unwrap(), id);
        assert_eq!(format!(" {id} ").parse::<ItemId>().unwrap(), id);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!("nope".parse::<VaultId>(), Err(ModelError::InvalidId));
    }

    #[test]
    fn bytes_round_trip() {
        let id = VaultId::new();
        assert_eq!(VaultId::from_bytes(*id.as_bytes()), id);
    }

    #[test]
    fn serializes_as_uuid_string() {
        let id = VaultId::from_bytes([0; 16]);
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"00000000-0000-0000-0000-000000000000\""
        );
    }
}
