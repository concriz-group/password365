//! Memorable passphrases from the EFF large wordlist (diceware-style).

use password365_crypto::secret::SecretString;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{GeneratorError, Result, entropy, pick, wordlist};

/// Minimum number of words.
pub const MIN_WORDS: usize = 3;
/// Maximum number of words.
pub const MAX_WORDS: usize = 20;
/// Maximum separator length in characters.
pub const MAX_SEPARATOR: usize = 3;

const DIGITS: [char; 10] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

/// How to build a passphrase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassphraseRecipe {
    /// Number of words.
    pub words: usize,
    /// Text placed between words.
    pub separator: String,
    /// Capitalize the first letter of every word.
    pub capitalize: bool,
    /// Append one random digit to one random word.
    pub include_number: bool,
}

impl Default for PassphraseRecipe {
    /// 5 capitalized words joined by `-` with a digit (~70 bits).
    fn default() -> Self {
        Self {
            words: 5,
            separator: "-".into(),
            capitalize: true,
            include_number: true,
        }
    }
}

impl PassphraseRecipe {
    /// Checks that the recipe is usable.
    pub fn validate(&self) -> Result<()> {
        if !(MIN_WORDS..=MAX_WORDS).contains(&self.words) {
            return Err(GeneratorError::InvalidRecipe(format!(
                "words must be {MIN_WORDS}..={MAX_WORDS}"
            )));
        }
        if self.separator.chars().count() > MAX_SEPARATOR {
            return Err(GeneratorError::InvalidRecipe("separator too long".into()));
        }
        Ok(())
    }

    /// Estimated entropy in bits.
    #[must_use]
    pub fn entropy_bits(&self) -> f64 {
        let words = entropy::bits(wordlist::EFF_LARGE_LEN, self.words);
        if self.include_number {
            words + entropy::bits(DIGITS.len(), 1) + entropy::bits(self.words, 1)
        } else {
            words
        }
    }

    /// Generates a passphrase following this recipe.
    pub fn generate(&self) -> Result<SecretString> {
        self.validate()?;
        let list = wordlist::eff_large();
        let mut words: Vec<Zeroizing<String>> = (0..self.words)
            .map(|_| pick::one(list).map(|w| Zeroizing::new(self.style(w))))
            .collect::<Result<_>>()?;

        if self.include_number {
            let positions: Vec<usize> = (0..words.len()).collect();
            let target = pick::one(&positions)?;
            words[target].push(pick::one(&DIGITS)?);
        }

        let joined = words
            .iter()
            .map(|w| w.as_str())
            .collect::<Vec<_>>()
            .join(&self.separator);
        Ok(SecretString::new(joined))
    }

    fn style(&self, word: &str) -> String {
        if !self.capitalize {
            return word.to_owned();
        }
        let mut chars = word.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(chars).collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shape() {
        // A few EFF words contain '-' ("t-shirt"), so count with a space.
        assert_eq!(PassphraseRecipe::default().separator, "-");
        let recipe = PassphraseRecipe {
            separator: " ".into(),
            ..PassphraseRecipe::default()
        };
        let phrase = recipe.generate().unwrap();
        let parts: Vec<&str> = phrase.expose().split(' ').collect();
        assert_eq!(parts.len(), 5);
        assert!(
            parts
                .iter()
                .all(|p| p.chars().next().unwrap().is_uppercase())
        );
        assert_eq!(
            phrase.expose().chars().filter(char::is_ascii_digit).count(),
            1
        );
    }

    #[test]
    fn plain_words_from_list() {
        let recipe = PassphraseRecipe {
            words: 4,
            separator: " ".into(),
            capitalize: false,
            include_number: false,
        };
        let phrase = recipe.generate().unwrap();
        for word in phrase.expose().split(' ') {
            assert!(wordlist::eff_large().contains(&word), "{word} not in list");
        }
    }

    #[test]
    fn empty_separator_is_allowed() {
        let recipe = PassphraseRecipe {
            separator: String::new(),
            ..PassphraseRecipe::default()
        };
        assert!(!recipe.generate().unwrap().expose().contains('-'));
    }

    #[test]
    fn rejects_bad_word_count() {
        assert!(
            PassphraseRecipe {
                words: 2,
                ..PassphraseRecipe::default()
            }
            .generate()
            .is_err()
        );
        assert!(
            PassphraseRecipe {
                words: 21,
                ..PassphraseRecipe::default()
            }
            .generate()
            .is_err()
        );
    }

    #[test]
    fn rejects_long_separator() {
        let r = PassphraseRecipe {
            separator: "----".into(),
            ..PassphraseRecipe::default()
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn entropy_estimates() {
        let plain = PassphraseRecipe {
            words: 6,
            include_number: false,
            ..PassphraseRecipe::default()
        };
        assert!((plain.entropy_bits() - 77.55).abs() < 0.01);
        let numbered = PassphraseRecipe {
            words: 6,
            ..PassphraseRecipe::default()
        };
        assert!(numbered.entropy_bits() > plain.entropy_bits());
    }

    #[test]
    fn style_capitalizes_and_handles_empty() {
        let r = PassphraseRecipe::default();
        assert_eq!(r.style("apple"), "Apple");
        assert_eq!(r.style(""), "");
    }
}
