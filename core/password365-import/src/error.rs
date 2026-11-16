use thiserror::Error;

/// Import / export errors.
#[derive(Debug, Error)]
pub enum ImportError {
    /// The CSV could not be read or written.
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    /// The CSV has no recognizable password column.
    #[error("not a password CSV: missing column '{0}'")]
    MissingColumn(&'static str),
    /// Another export format could not be read.
    #[error("{0}")]
    Format(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, ImportError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages() {
        assert_eq!(
            ImportError::MissingColumn("password").to_string(),
            "not a password CSV: missing column 'password'"
        );
    }
}
