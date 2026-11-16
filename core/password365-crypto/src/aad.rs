//! Builder for unambiguous associated data (AAD).
//!
//! AAD binds a ciphertext to its context (which vault, which item, which
//! version …) so ciphertexts cannot be swapped around by someone with write
//! access to storage. Every part is length-prefixed, so `("ab","c")` and
//! `("a","bc")` can never collide.

/// Incrementally builds an AAD byte string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aad(Vec<u8>);

impl Aad {
    /// Starts a new AAD with a domain-separation label, e.g. `"quillock/v1/item-details"`.
    #[must_use]
    pub fn new(domain: &str) -> Self {
        Self(Vec::new()).bytes(domain.as_bytes())
    }

    /// Appends a length-prefixed byte field.
    ///
    /// # Panics
    /// Panics if `part` is larger than 4 GiB (never the case for AAD fields).
    #[must_use]
    pub fn bytes(mut self, part: &[u8]) -> Self {
        let len = u32::try_from(part.len()).expect("AAD part larger than 4 GiB");
        self.0.extend_from_slice(&len.to_be_bytes());
        self.0.extend_from_slice(part);
        self
    }

    /// Appends a big-endian `u64` field.
    #[must_use]
    pub fn u64(self, value: u64) -> Self {
        self.bytes(&value.to_be_bytes())
    }

    /// Returns the encoded AAD.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_is_length_prefixed() {
        assert_eq!(Aad::new("ab").as_bytes(), &[0, 0, 0, 2, b'a', b'b']);
    }

    #[test]
    fn fields_cannot_be_shifted_between_parts() {
        let a = Aad::new("d").bytes(b"ab").bytes(b"c");
        let b = Aad::new("d").bytes(b"a").bytes(b"bc");
        assert_ne!(a, b);
    }

    #[test]
    fn different_domains_differ() {
        assert_ne!(Aad::new("x").bytes(b"1"), Aad::new("y").bytes(b"1"));
    }

    #[test]
    fn u64_is_encoded_big_endian() {
        let aad = Aad::new("").u64(1);
        assert_eq!(
            aad.as_bytes(),
            &[0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 1]
        );
    }

    #[test]
    fn same_inputs_are_deterministic() {
        assert_eq!(
            Aad::new("d").u64(3).bytes(b"x"),
            Aad::new("d").u64(3).bytes(b"x")
        );
    }
}
