//! Detects records rolled back, removed or added behind the app's back.
//!
//! Someone who can write the vault file but has no keys cannot read or forge
//! records, but could put back an older copy of one (a revoked device, an old
//! password), or delete one. So the store keeps the sum (mod 2^256) of a keyed
//! MAC of every record, updated in the same write as the record, and checked
//! on unlock. Without the key nobody can compute the sum for another set of
//! records. Putting back an older copy of the *whole* file is not detectable
//! without state kept elsewhere; syncing with the other devices brings newer
//! items back.

use hmac::{Hmac, KeyInit, Mac};
use password365_crypto::key::SymmetricKey;
use sha2::Sha256;

/// The sum of the records' MACs.
pub(crate) type Sum = [u8; 32];

/// The MAC key, derived from the storage key.
pub(crate) fn key(
    storage_key: &SymmetricKey,
    store_id: &[u8],
) -> Result<SymmetricKey, password365_crypto::CryptoError> {
    password365_crypto::hkdf::derive_key(
        storage_key.expose(),
        store_id,
        b"password365/v1/record-integrity",
    )
}

/// One record's contribution.
pub(crate) fn element(key: &SymmetricKey, id: &[u8], blob: &[u8]) -> Sum {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key.expose()).expect("any key length");
    mac.update(&u32::try_from(id.len()).unwrap_or(u32::MAX).to_be_bytes());
    mac.update(id);
    mac.update(blob);
    mac.finalize().into_bytes().into()
}

/// `a + b` (mod 2^256), little-endian.
pub(crate) fn add(a: &Sum, b: &Sum) -> Sum {
    let mut out = [0u8; 32];
    let mut carry = 0u16;
    for i in 0..32 {
        let v = u16::from(a[i]) + u16::from(b[i]) + carry;
        out[i] = (v & 0xff) as u8;
        carry = v >> 8;
    }
    out
}

/// `a - b` (mod 2^256), little-endian.
pub(crate) fn sub(a: &Sum, b: &Sum) -> Sum {
    let mut out = [0u8; 32];
    let mut borrow = 0i16;
    for i in 0..32 {
        let mut v = i16::from(a[i]) - i16::from(b[i]) - borrow;
        borrow = 0;
        if v < 0 {
            v += 256;
            borrow = 1;
        }
        out[i] = u8::try_from(v).unwrap_or(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_add_and_remove_in_any_order() {
        let k = SymmetricKey::generate().unwrap();
        let (a, b) = (element(&k, b"1", b"x"), element(&k, b"2", b"y"));
        let both = add(&add(&[0; 32], &a), &b);
        assert_eq!(both, add(&add(&[0; 32], &b), &a));
        assert_eq!(sub(&both, &a), b);
        assert_eq!(sub(&sub(&both, &b), &a), [0; 32]);
        assert_ne!(element(&k, b"1", b"x"), element(&k, b"1", b"z"));
        assert_ne!(
            element(&k, b"1", b"x"),
            element(&SymmetricKey::generate().unwrap(), b"1", b"x")
        );
    }
}
