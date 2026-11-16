//! Access to the operating system CSPRNG.
//!
//! All randomness in Password365 flows through this module so it can be audited
//! (and so nothing accidentally uses a non-cryptographic generator).

use crate::{CryptoError, Result};

/// Fills `buf` with cryptographically secure random bytes.
pub fn fill(buf: &mut [u8]) -> Result<()> {
    getrandom::fill(buf).map_err(|_| CryptoError::Random)
}

/// Returns an array of `N` cryptographically secure random bytes.
pub fn bytes<const N: usize>() -> Result<[u8; N]> {
    let mut out = [0u8; N];
    fill(&mut out)?;
    Ok(out)
}

/// Returns a uniformly distributed integer in `0..bound`.
///
/// Uses rejection sampling so there is no modulo bias.
///
/// # Panics
/// Panics if `bound` is zero.
pub fn uniform_below(bound: u32) -> Result<u32> {
    uniform_below_with(bound, || bytes::<4>().map(u32::from_le_bytes))
}

/// Core of [`uniform_below`], with an injectable source so the rejection path
/// can be tested deterministically.
fn uniform_below_with(bound: u32, mut next: impl FnMut() -> Result<u32>) -> Result<u32> {
    assert!(bound > 0, "bound must be positive");
    // Largest multiple of `bound` that fits in u32; values at or above it are rejected.
    let zone = u32::MAX - (u32::MAX % bound);
    loop {
        let candidate = next()?;
        if candidate < zone {
            return Ok(candidate % bound);
        }
    }
}

/// Shuffles `items` in place with an unbiased Fisher–Yates shuffle.
pub fn shuffle<T>(items: &mut [T]) -> Result<()> {
    for i in (1..items.len()).rev() {
        let bound = u32::try_from(i + 1).map_err(|_| CryptoError::Malformed("slice too long"))?;
        let j = uniform_below(bound)? as usize;
        items.swap(i, j);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_produces_non_constant_output() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        fill(&mut a).unwrap();
        fill(&mut b).unwrap();
        assert_ne!(a, b);
        assert_ne!(a, [0u8; 32]);
    }

    #[test]
    fn bytes_returns_requested_length() {
        let out: [u8; 24] = bytes().unwrap();
        assert_eq!(out.len(), 24);
    }

    #[test]
    fn uniform_below_stays_in_range() {
        for bound in [1, 2, 3, 7, 10, 62, 7776] {
            for _ in 0..200 {
                assert!(uniform_below(bound).unwrap() < bound);
            }
        }
    }

    #[test]
    fn uniform_below_one_is_always_zero() {
        assert_eq!(uniform_below(1).unwrap(), 0);
    }

    #[test]
    #[should_panic(expected = "bound must be positive")]
    fn uniform_below_zero_panics() {
        let _ = uniform_below(0);
    }

    #[test]
    fn uniform_below_rejects_values_in_biased_zone() {
        // bound = 3 → zone = u32::MAX - (u32::MAX % 3) = u32::MAX (since MAX % 3 == 0)
        // use bound = 10: zone = MAX - 5; feed a rejected value first.
        let mut values = vec![5u32, u32::MAX].into_iter();
        let result = uniform_below_with(10, || Ok(values.next_back().unwrap())).unwrap();
        // first draw (u32::MAX) is rejected, second (5) accepted
        assert_eq!(result, 5);
    }

    #[test]
    fn uniform_below_propagates_source_errors() {
        let result = uniform_below_with(10, || Err(CryptoError::Random));
        assert_eq!(result, Err(CryptoError::Random));
    }

    #[test]
    fn uniform_below_is_roughly_uniform() {
        let mut counts = [0u32; 4];
        for _ in 0..4000 {
            counts[uniform_below(4).unwrap() as usize] += 1;
        }
        for c in counts {
            assert!((800..1200).contains(&c), "skewed distribution: {counts:?}");
        }
    }

    #[test]
    fn shuffle_keeps_all_elements() {
        let mut items: Vec<u32> = (0..50).collect();
        shuffle(&mut items).unwrap();
        let mut sorted = items.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_changes_order_eventually() {
        let original: Vec<u32> = (0..20).collect();
        let changed = (0..10).any(|_| {
            let mut items = original.clone();
            shuffle(&mut items).unwrap();
            items != original
        });
        assert!(changed);
    }

    #[test]
    fn shuffle_handles_empty_and_single() {
        let mut empty: [u8; 0] = [];
        shuffle(&mut empty).unwrap();
        let mut one = [9];
        shuffle(&mut one).unwrap();
        assert_eq!(one, [9]);
    }
}
