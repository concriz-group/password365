//! Character classes for random passwords.

use serde::{Deserialize, Serialize};

/// Characters that are easily confused when read or typed.
pub const AMBIGUOUS: &str = "Il1O0o|`'\"";

/// A class of characters a password may draw from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CharClass {
    /// a–z
    Lowercase,
    /// A–Z
    Uppercase,
    /// 0–9
    Digits,
    /// Printable ASCII symbols.
    Symbols,
}

impl CharClass {
    /// Every class.
    pub const ALL: [Self; 4] = [
        Self::Lowercase,
        Self::Uppercase,
        Self::Digits,
        Self::Symbols,
    ];

    /// All characters in this class.
    #[must_use]
    pub const fn chars(self) -> &'static str {
        match self {
            Self::Lowercase => "abcdefghijklmnopqrstuvwxyz",
            Self::Uppercase => "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            Self::Digits => "0123456789",
            Self::Symbols => "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~",
        }
    }

    /// Characters in this class, optionally without ambiguous ones.
    #[must_use]
    pub fn alphabet(self, exclude_ambiguous: bool) -> Vec<char> {
        self.chars()
            .chars()
            .filter(|c| !(exclude_ambiguous && AMBIGUOUS.contains(*c)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_sizes() {
        assert_eq!(CharClass::Lowercase.chars().len(), 26);
        assert_eq!(CharClass::Uppercase.chars().len(), 26);
        assert_eq!(CharClass::Digits.chars().len(), 10);
        assert_eq!(CharClass::Symbols.chars().len(), 32);
    }

    #[test]
    fn classes_do_not_overlap() {
        for a in CharClass::ALL {
            for b in CharClass::ALL {
                if a != b {
                    assert!(
                        !a.chars().chars().any(|c| b.chars().contains(c)),
                        "{a:?} vs {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn ambiguous_characters_can_be_excluded() {
        let lower = CharClass::Lowercase.alphabet(true);
        assert!(!lower.contains(&'l') && !lower.contains(&'o'));
        assert_eq!(lower.len(), 24);
        assert_eq!(CharClass::Digits.alphabet(true).len(), 8);
        assert_eq!(CharClass::Digits.alphabet(false).len(), 10);
    }
}
