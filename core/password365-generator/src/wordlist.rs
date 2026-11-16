//! Embedded EFF large wordlist (7,776 words, CC BY 3.0 US).

use std::sync::OnceLock;

const EFF_LARGE_RAW: &str = include_str!("../data/eff_large_wordlist.txt");

/// Number of words in the EFF large list.
pub const EFF_LARGE_LEN: usize = 7776;

/// The EFF large wordlist (dice numbers stripped).
#[must_use]
pub fn eff_large() -> &'static [&'static str] {
    static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| parse(EFF_LARGE_RAW))
}

/// Parses `"<dice>\t<word>"` lines into words.
fn parse(raw: &'static str) -> Vec<&'static str> {
    raw.lines()
        .filter_map(|line| line.split('\t').nth(1))
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn has_7776_unique_words() {
        let words = eff_large();
        assert_eq!(words.len(), EFF_LARGE_LEN);
        assert_eq!(words.iter().collect::<HashSet<_>>().len(), EFF_LARGE_LEN);
    }

    #[test]
    fn words_are_lowercase_ascii() {
        assert!(
            eff_large()
                .iter()
                .all(|w| w.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
        );
    }

    #[test]
    fn parse_skips_malformed_lines() {
        assert_eq!(parse("11111\tabacus\nbroken\n11112\t \n"), vec!["abacus"]);
    }
}
