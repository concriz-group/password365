use thiserror::Error;

/// Validation errors for model types.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    /// A required name or title was empty or whitespace.
    #[error("{0} must not be empty")]
    Empty(&'static str),
    /// A text field exceeded its maximum length.
    #[error("{field} is longer than {max} characters")]
    TooLong {
        /// Field name.
        field: &'static str,
        /// Maximum allowed characters.
        max: usize,
    },
    /// An identifier string could not be parsed.
    #[error("invalid identifier")]
    InvalidId,
    /// A value breaks a rule (the message says which).
    #[error("{0}")]
    Invalid(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, ModelError>;

/// Trims `value` and checks it is non-empty and at most `max` characters.
pub(crate) fn required_text(value: &str, field: &'static str, max: usize) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ModelError::Empty(field));
    }
    if trimmed.chars().count() > max {
        return Err(ModelError::TooLong { field, max });
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_text_trims() {
        assert_eq!(required_text("  hi ", "title", 10).unwrap(), "hi");
    }

    #[test]
    fn required_text_rejects_blank() {
        assert_eq!(
            required_text("   ", "title", 10),
            Err(ModelError::Empty("title"))
        );
    }

    #[test]
    fn required_text_rejects_too_long() {
        assert_eq!(
            required_text("abcdef", "name", 5),
            Err(ModelError::TooLong {
                field: "name",
                max: 5
            })
        );
    }

    #[test]
    fn required_text_counts_characters_not_bytes() {
        assert!(required_text("ééééé", "name", 5).is_ok());
    }

    #[test]
    fn messages() {
        assert_eq!(
            ModelError::Empty("title").to_string(),
            "title must not be empty"
        );
        assert_eq!(
            ModelError::TooLong {
                field: "name",
                max: 3
            }
            .to_string(),
            "name is longer than 3 characters"
        );
        assert_eq!(ModelError::InvalidId.to_string(), "invalid identifier");
    }
}
