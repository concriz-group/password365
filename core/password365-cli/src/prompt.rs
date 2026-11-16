//! User interaction (hidden password entry, yes/no questions).
//!
//! Commands talk to a [`Prompter`] trait so they can be tested with scripted answers.

use std::io::{self, BufRead, Write};

use password365_crypto::secret::SecretString;

/// Source of user input.
pub trait Prompter {
    /// Reads a secret without echoing it.
    fn secret(&mut self, label: &str) -> io::Result<SecretString>;
    /// Asks a yes/no question (default: no).
    fn confirm(&mut self, question: &str) -> io::Result<bool>;
}

/// Interactive terminal prompter.
pub struct TerminalPrompter {
    /// Read every secret from stdin (one per line) instead of the TTY.
    pub password_stdin: bool,
}

impl Prompter for TerminalPrompter {
    fn secret(&mut self, label: &str) -> io::Result<SecretString> {
        if self.password_stdin {
            return read_line(&mut io::stdin().lock()).map(SecretString::new);
        }
        rpassword::prompt_password(format!("{label}: ")).map(SecretString::new)
    }

    fn confirm(&mut self, question: &str) -> io::Result<bool> {
        eprint!("{question} [y/N] ");
        io::stderr().flush()?;
        Ok(is_yes(&read_line(&mut io::stdin().lock())?))
    }
}

/// Reads one line without its trailing newline.
fn read_line(reader: &mut impl BufRead) -> io::Result<String> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let trimmed_len = line.trim_end_matches(['\r', '\n']).len();
    line.truncate(trimmed_len);
    Ok(line)
}

fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Prompter returning pre-recorded answers (tests).
#[cfg(test)]
pub struct ScriptedPrompter {
    answers: std::collections::VecDeque<String>,
}

#[cfg(test)]
impl ScriptedPrompter {
    /// Creates a prompter that replays `answers` in order.
    pub fn new(answers: &[&str]) -> Self {
        Self {
            answers: answers.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn next(&mut self) -> io::Result<String> {
        self.answers
            .pop_front()
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "no scripted answer left"))
    }
}

#[cfg(test)]
impl Prompter for ScriptedPrompter {
    fn secret(&mut self, _label: &str) -> io::Result<SecretString> {
        self.next().map(SecretString::new)
    }

    fn confirm(&mut self, _question: &str) -> io::Result<bool> {
        self.next().map(|a| is_yes(&a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_line_strips_newlines_only() {
        let mut input = io::Cursor::new(" pass word \r\nnext");
        assert_eq!(read_line(&mut input).unwrap(), " pass word ");
    }

    #[test]
    fn yes_answers() {
        assert!(is_yes("y"));
        assert!(is_yes(" YES "));
        assert!(!is_yes(""));
        assert!(!is_yes("no"));
    }

    #[test]
    fn scripted_prompter_replays_answers() {
        let mut p = ScriptedPrompter::new(&["pw", "y"]);
        assert_eq!(p.secret("x").unwrap().expose(), "pw");
        assert!(p.confirm("q").unwrap());
        assert!(p.secret("x").is_err());
    }
}
