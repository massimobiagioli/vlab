//! Display names shared by laboratories, agents, and projects (A-23).

use std::fmt;

/// Longest accepted name, in characters after trimming.
pub const NAME_MAX_CHARS: usize = 100;

/// A trimmed, non-empty, single-line display name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);

impl Name {
    pub fn parse(input: &str) -> Result<Self, NameError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(NameError::Empty);
        }
        if trimmed.chars().count() > NAME_MAX_CHARS {
            return Err(NameError::TooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(NameError::ControlCharacters);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("a name is required.")]
    Empty,
    #[error("a name may contain at most 100 characters.")]
    TooLong,
    #[error("a name must not contain control characters.")]
    ControlCharacters,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_bounded_by_characters() {
        assert_eq!(
            Name::parse("  Payments Platform  ").unwrap().as_str(),
            "Payments Platform"
        );
        assert_eq!(
            Name::parse(&"é".repeat(100)).unwrap().as_str(),
            "é".repeat(100)
        );
        assert_eq!(Name::parse(&"a".repeat(101)), Err(NameError::TooLong));
    }

    #[test]
    fn empty_or_control_bearing_names_are_rejected() {
        for bad in ["", "   ", "\n", "Pay\u{7}ments", "two\nlines", "tab\there"] {
            assert!(Name::parse(bad).is_err(), "{bad:?}");
        }
    }
}
