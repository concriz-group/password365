//! Last-writer-wins register: a value plus the stamp of the write that set it.

use serde::{Deserialize, Serialize};

use crate::Hlc;

/// A value and the HLC stamp of its last write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lww<T> {
    /// Current value.
    pub value: T,
    /// When (and where) it was written.
    pub stamp: Hlc,
}

impl<T: Clone + PartialEq> Lww<T> {
    /// A register holding `value` written at `stamp`.
    pub const fn new(value: T, stamp: Hlc) -> Self {
        Self { value, stamp }
    }

    /// Writes `value` at `stamp` if it differs; returns whether it changed.
    pub fn set(&mut self, value: T, stamp: Hlc) -> bool {
        if self.value == value {
            return false;
        }
        self.value = value;
        self.stamp = stamp;
        true
    }

    /// The register with the newer stamp (stamps are unique per write).
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        if other.stamp > self.stamp {
            other.clone()
        } else {
            self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeviceId;

    fn at(wall_ms: i64) -> Hlc {
        Hlc {
            wall_ms,
            counter: 0,
            device: DeviceId::from_bytes([1; 16]),
        }
    }

    #[test]
    fn set_only_restamps_on_change() {
        let mut r = Lww::new("a", at(1));
        assert!(!r.set("a", at(2)));
        assert_eq!(r.stamp, at(1));
        assert!(r.set("b", at(3)));
        assert_eq!(r, Lww::new("b", at(3)));
    }

    #[test]
    fn merge_keeps_the_newer_write_either_way() {
        let old = Lww::new(1, at(1));
        let new = Lww::new(2, at(2));
        assert_eq!(old.merge(&new), new);
        assert_eq!(new.merge(&old), new);
        assert_eq!(new.merge(&new), new);
    }
}
