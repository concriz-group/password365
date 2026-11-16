//! Auto-type (from KeePass): type an item's username, password, one-time
//! code and keys into another program's window, following a sequence such as
//! `{USERNAME}{TAB}{PASSWORD}{ENTER}`.
//!
//! The sequence and the windows an item belongs to are stored in two of the
//! item's custom fields ([`SEQUENCE_LABEL`], [`WINDOW_LABEL`]), so they sync
//! like everything else. This module only plans the keystrokes; the desktop
//! app performs them. Secrets stay in [`SecretString`]s until typed.
//!
//! Placeholders: `{USERNAME}` `{PASSWORD}` `{TOTP}` `{TITLE}` `{URL}`,
//! `{S:field label}` (any field), keys `{TAB}` `{ENTER}` `{SPACE}`
//! `{BACKSPACE}` `{ESC}` `{UP}` `{DOWN}` `{LEFT}` `{RIGHT}`, `{DELAY 500}` (pause
//! in ms), `{DELAY=50}` (pause between keystrokes from then on), `{{}` and
//! `{}}` for literal braces. Anything else is typed as is.

use password365_crypto::secret::SecretString;
use password365_crypto::totp::Totp;
use password365_model::{Item, Timestamp};
use serde::Serialize;

use crate::{AppError, AppService, Result};

/// Custom field holding the item's sequence.
pub const SEQUENCE_LABEL: &str = "auto-type sequence";
/// Custom field holding the window titles the item types into (one pattern
/// per line, `*` matches anything).
pub const WINDOW_LABEL: &str = "auto-type window";
/// Used when the item has no sequence of its own.
pub const DEFAULT_SEQUENCE: &str = "{USERNAME}{TAB}{PASSWORD}{ENTER}";
/// Pause between keystrokes unless the sequence sets one.
pub const DEFAULT_KEY_DELAY_MS: u32 = 10;
/// Longest pause a sequence may ask for.
const MAX_DELAY_MS: u32 = 60_000;

/// A special key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Key {
    /// Tab.
    Tab,
    /// Enter / Return.
    Enter,
    /// Space bar.
    Space,
    /// Backspace.
    Backspace,
    /// Escape.
    Escape,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
}

/// One thing to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Type this text.
    Text(SecretString),
    /// Press this key.
    Key(Key),
    /// Wait this many milliseconds.
    Pause(u32),
    /// From now on, wait this many milliseconds between keystrokes.
    KeyDelay(u32),
}

/// What a placeholder refers to, before values are filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Literal(String),
    Field(Source),
    Key(Key),
    Pause(u32),
    KeyDelay(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    Username,
    Password,
    Totp,
    Title,
    Url,
    Custom(String),
}

/// Checks a sequence typed by the user.
pub fn validate(sequence: &str) -> Result<()> {
    parse(sequence).map(|_| ())
}

fn parse(sequence: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut rest = sequence;
    while !rest.is_empty() {
        let Some(open) = rest.find('{') else {
            tokens.push(Token::Literal(rest.to_owned()));
            break;
        };
        if open > 0 {
            tokens.push(Token::Literal(rest[..open].to_owned()));
        }
        rest = &rest[open..];
        // `{}}` is a literal closing brace.
        let close = if rest.starts_with("{}}") {
            2
        } else {
            rest.find('}')
                .ok_or_else(|| invalid("a { is never closed"))?
        };
        tokens.push(placeholder(&rest[1..close])?);
        rest = &rest[close + 1..];
    }
    Ok(tokens)
}

fn placeholder(name: &str) -> Result<Token> {
    let upper = name.to_ascii_uppercase();
    if let Some(label) = name.strip_prefix("S:").or_else(|| name.strip_prefix("s:")) {
        return Ok(Token::Field(Source::Custom(label.to_owned())));
    }
    if let Some(ms) = upper.strip_prefix("DELAY=") {
        return Ok(Token::KeyDelay(delay(ms)?));
    }
    if let Some(ms) = upper.strip_prefix("DELAY ") {
        return Ok(Token::Pause(delay(ms)?));
    }
    Ok(match upper.as_str() {
        "USERNAME" => Token::Field(Source::Username),
        "PASSWORD" => Token::Field(Source::Password),
        "TOTP" => Token::Field(Source::Totp),
        "TITLE" => Token::Field(Source::Title),
        "URL" => Token::Field(Source::Url),
        "TAB" => Token::Key(Key::Tab),
        "ENTER" => Token::Key(Key::Enter),
        "SPACE" => Token::Key(Key::Space),
        "BACKSPACE" | "BS" => Token::Key(Key::Backspace),
        "ESC" => Token::Key(Key::Escape),
        "UP" => Token::Key(Key::Up),
        "DOWN" => Token::Key(Key::Down),
        "LEFT" => Token::Key(Key::Left),
        "RIGHT" => Token::Key(Key::Right),
        "{" | "}" => Token::Literal(upper),
        _ => return Err(invalid(&format!("unknown placeholder {{{name}}}"))),
    })
}

fn delay(ms: &str) -> Result<u32> {
    ms.trim()
        .parse::<u32>()
        .ok()
        .filter(|&ms| ms <= MAX_DELAY_MS)
        .ok_or_else(|| invalid("a delay must be a number of milliseconds up to 60000"))
}

fn invalid(problem: &str) -> AppError {
    AppError::Validation(format!("auto-type sequence: {problem}"))
}

/// The keystrokes for `item` at `now`.
pub fn steps(item: &Item, now: Timestamp) -> Result<Vec<Step>> {
    let sequence = field(item, SEQUENCE_LABEL)
        .map(|s| s.expose().trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_SEQUENCE.to_owned());
    let mut steps = vec![Step::KeyDelay(DEFAULT_KEY_DELAY_MS)];
    for token in parse(&sequence)? {
        steps.push(match token {
            Token::Literal(text) => Step::Text(text.into()),
            Token::Field(source) => Step::Text(value(item, &source, now)?),
            Token::Key(key) => Step::Key(key),
            Token::Pause(ms) => Step::Pause(ms),
            Token::KeyDelay(ms) => Step::KeyDelay(ms),
        });
    }
    Ok(steps)
}

fn value(item: &Item, source: &Source, now: Timestamp) -> Result<SecretString> {
    let text = |s: Option<&String>| SecretString::from(s.cloned().unwrap_or_default());
    Ok(match source {
        Source::Username => text(item.overview.username.as_ref()),
        Source::Password => item.details.password.clone().unwrap_or_default(),
        Source::Title => item.overview.title.clone().into(),
        Source::Url => text(item.overview.urls.first()),
        Source::Custom(label) => field(item, label).cloned().unwrap_or_default(),
        Source::Totp => match &item.details.totp {
            Some(secret) => {
                let seconds = u64::try_from(now.as_millis() / 1000).unwrap_or_default();
                Totp::parse(secret.expose())?.code_at(seconds).code.into()
            }
            None => SecretString::default(),
        },
    })
}

fn field<'a>(item: &'a Item, label: &str) -> Option<&'a SecretString> {
    item.details
        .custom_fields
        .iter()
        .find(|f| {
            f.label.eq_ignore_ascii_case(label) && !crate::field_layout::is_internal(&f.label)
        })
        .map(|f| &f.value)
}

/// Whether `item` belongs to a window titled `title`: one of its window
/// patterns matches, or (without patterns) the window title contains the
/// item's title, as `KeePass` does.
#[must_use]
pub fn matches_window(item: &Item, title: &str) -> bool {
    let title = title.to_lowercase();
    if let Some(patterns) = field(item, WINDOW_LABEL) {
        return patterns
            .expose()
            .lines()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .any(|p| wildcard(&p.to_lowercase(), &title));
    }
    let own = item.overview.title.trim().to_lowercase();
    own.chars().count() >= 3 && title.contains(&own)
}

/// `*` matches any run of characters; everything else literally.
fn wildcard(pattern: &str, text: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// An item that can type into the current window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutoTypeCandidate {
    /// Item id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Username, if any.
    pub username: Option<String>,
}

impl AppService {
    /// The keystrokes for one item (secrets included: perform them, then drop them).
    pub fn auto_type_steps(&mut self, id: &str) -> Result<Vec<Step>> {
        let item = self.load_item(id)?;
        steps(&item, Timestamp::now())
    }

    /// Items (outside the Archive) that belong to a window titled `title`.
    pub fn auto_type_candidates(&mut self, title: &str) -> Result<Vec<AutoTypeCandidate>> {
        let session = self.session()?;
        let mut found = Vec::new();
        for summary in session
            .items()
            .into_iter()
            .filter(|s| crate::service::active(s))
        {
            let item = session.item(summary.id)?;
            let typable = item.details.password.is_some() || item.overview.username.is_some();
            if typable && matches_window(&item, title) {
                found.push(AutoTypeCandidate {
                    id: item.id.to_string(),
                    title: item.overview.title.clone(),
                    username: item.overview.username.clone(),
                });
            }
        }
        found.sort_by(|a, b| a.title.cmp(&b.title));
        Ok(found)
    }
}

#[cfg(test)]
#[path = "auto_type_tests.rs"]
mod tests;
