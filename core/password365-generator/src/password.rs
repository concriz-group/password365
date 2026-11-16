//! Random character passwords and PINs.

use password365_crypto::random;
use password365_crypto::secret::SecretString;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::charset::CharClass;
use crate::{GeneratorError, Result, entropy, pick};

/// Minimum allowed length.
pub const MIN_LENGTH: usize = 4;
/// Maximum allowed length.
pub const MAX_LENGTH: usize = 128;

/// How to build a random password.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordRecipe {
    /// Number of characters.
    pub length: usize,
    /// Classes to draw from; at least one character of each is guaranteed.
    pub classes: Vec<CharClass>,
    /// Skip easily confused characters (l, 1, O, 0 …).
    pub exclude_ambiguous: bool,
}

impl Default for PasswordRecipe {
    /// 20 characters from all classes (~130 bits).
    fn default() -> Self {
        Self {
            length: 20,
            classes: CharClass::ALL.to_vec(),
            exclude_ambiguous: false,
        }
    }
}

impl PasswordRecipe {
    /// A numeric PIN of `length` digits.
    #[must_use]
    pub fn pin(length: usize) -> Self {
        Self {
            length,
            classes: vec![CharClass::Digits],
            exclude_ambiguous: false,
        }
    }

    /// Checks that the recipe can produce a password.
    pub fn validate(&self) -> Result<()> {
        if !(MIN_LENGTH..=MAX_LENGTH).contains(&self.length) {
            return Err(invalid(format!(
                "length must be {MIN_LENGTH}..={MAX_LENGTH}"
            )));
        }
        if self.classes.is_empty() {
            return Err(invalid("at least one character class is required".into()));
        }
        if has_duplicates(&self.classes) {
            return Err(invalid("character classes must not repeat".into()));
        }
        if self.length < self.classes.len() {
            return Err(invalid(
                "length is shorter than the number of classes".into(),
            ));
        }
        Ok(())
    }

    /// Union of all allowed characters.
    #[must_use]
    pub fn alphabet(&self) -> Vec<char> {
        self.classes
            .iter()
            .flat_map(|c| c.alphabet(self.exclude_ambiguous))
            .collect()
    }

    /// Estimated entropy in bits.
    #[must_use]
    pub fn entropy_bits(&self) -> f64 {
        entropy::bits(self.alphabet().len(), self.length)
    }

    /// Generates a password following this recipe.
    pub fn generate(&self) -> Result<SecretString> {
        self.validate()?;
        let alphabet = self.alphabet();
        let mut chars: Zeroizing<Vec<char>> = Zeroizing::new(Vec::with_capacity(self.length));

        // One guaranteed character per class …
        for class in &self.classes {
            chars.push(pick::one(&class.alphabet(self.exclude_ambiguous))?);
        }
        // … the rest from the full alphabet, then shuffle so positions are random.
        while chars.len() < self.length {
            chars.push(pick::one(&alphabet)?);
        }
        random::shuffle(&mut chars)?;
        Ok(SecretString::new(chars.iter().collect()))
    }
}

fn has_duplicates(classes: &[CharClass]) -> bool {
    classes
        .iter()
        .enumerate()
        .any(|(i, c)| classes[i + 1..].contains(c))
}

fn invalid(message: String) -> GeneratorError {
    GeneratorError::InvalidRecipe(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charset::AMBIGUOUS;

    #[test]
    fn default_recipe_generates_valid_password() {
        let recipe = PasswordRecipe::default();
        let pw = recipe.generate().unwrap();
        assert_eq!(pw.expose().chars().count(), 20);
        assert!(recipe.entropy_bits() > 128.0);
    }

    #[test]
    fn contains_every_requested_class() {
        let recipe = PasswordRecipe {
            length: 4,
            ..PasswordRecipe::default()
        };
        for _ in 0..100 {
            let pw = recipe.generate().unwrap();
            for class in CharClass::ALL {
                assert!(
                    pw.expose().chars().any(|c| class.chars().contains(c)),
                    "{class:?} missing"
                );
            }
        }
    }

    #[test]
    fn only_uses_allowed_characters() {
        let recipe = PasswordRecipe {
            length: 64,
            classes: vec![CharClass::Lowercase, CharClass::Digits],
            exclude_ambiguous: true,
        };
        let alphabet = recipe.alphabet();
        let pw = recipe.generate().unwrap();
        assert!(
            pw.expose()
                .chars()
                .all(|c| alphabet.contains(&c) && !AMBIGUOUS.contains(c))
        );
    }

    #[test]
    fn pin_is_digits_only() {
        let pin = PasswordRecipe::pin(6).generate().unwrap();
        assert_eq!(pin.expose().len(), 6);
        assert!(pin.expose().chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn passwords_are_unique() {
        let recipe = PasswordRecipe::default();
        assert_ne!(recipe.generate().unwrap(), recipe.generate().unwrap());
    }

    #[test]
    fn rejects_bad_length() {
        assert!(
            PasswordRecipe {
                length: 3,
                ..PasswordRecipe::default()
            }
            .generate()
            .is_err()
        );
        assert!(
            PasswordRecipe {
                length: 129,
                ..PasswordRecipe::default()
            }
            .generate()
            .is_err()
        );
    }

    #[test]
    fn rejects_empty_classes() {
        let r = PasswordRecipe {
            classes: vec![],
            ..PasswordRecipe::default()
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn rejects_repeated_classes() {
        let adjacent = PasswordRecipe {
            classes: vec![CharClass::Digits, CharClass::Digits],
            ..PasswordRecipe::default()
        };
        let apart = PasswordRecipe {
            classes: vec![CharClass::Digits, CharClass::Lowercase, CharClass::Digits],
            ..PasswordRecipe::default()
        };
        assert!(adjacent.validate().is_err());
        assert!(apart.validate().is_err());
    }

    #[test]
    fn rejects_length_shorter_than_class_count() {
        let r = PasswordRecipe {
            length: 4,
            classes: CharClass::ALL.to_vec(),
            ..PasswordRecipe::default()
        };
        assert!(r.validate().is_ok());
        let mut five = CharClass::ALL.to_vec();
        five.push(CharClass::Digits);
        assert!(
            PasswordRecipe {
                length: 4,
                classes: five,
                ..PasswordRecipe::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn entropy_matches_alphabet_size() {
        let pin = PasswordRecipe::pin(10);
        assert!((pin.entropy_bits() - 10.0 * 10f64.log2()).abs() < 1e-9);
    }
}
