//! Uniform random selection shared by the password and passphrase generators.

use password365_crypto::random;

use crate::{GeneratorError, Result};

/// Picks one element uniformly at random.
pub(crate) fn one<T: Copy>(items: &[T]) -> Result<T> {
    let bound = u32::try_from(items.len())
        .ok()
        .filter(|&n| n > 0)
        .ok_or_else(|| GeneratorError::InvalidRecipe("nothing to pick from".into()))?;
    Ok(items[random::uniform_below(bound)? as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_from_slice() {
        let items = ['a', 'b', 'c'];
        for _ in 0..50 {
            assert!(items.contains(&one(&items).unwrap()));
        }
    }

    #[test]
    fn single_element_is_always_picked() {
        assert_eq!(one(&[7]).unwrap(), 7);
    }

    #[test]
    fn empty_slice_is_an_error() {
        let empty: [u8; 0] = [];
        assert!(one(&empty).is_err());
    }
}
