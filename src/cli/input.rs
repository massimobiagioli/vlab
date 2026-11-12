//! `--new-request` input handling (WP-5-T9).

use std::io::{self, IsTerminal, Read};

use crate::app::AppError;
use crate::domain::budget::ExecutionLimits;

/// Reads literal text or bounded stdin. Rejects simultaneous sources, empty
/// input, invalid UTF-8, and oversized input before any database work.
pub fn request_text(positional: Option<&str>, limits: ExecutionLimits) -> Result<String, AppError> {
    let max = limits.request_max_bytes;
    match positional {
        Some(text) => {
            if !io::stdin().is_terminal() {
                return Err(AppError::Usage {
                    message: "provide either literal request text or standard input, not both."
                        .to_owned(),
                });
            }
            finish(text.to_owned(), max)
        }
        None => {
            if io::stdin().is_terminal() {
                return Err(AppError::Usage {
                    message: "a request is required.".to_owned(),
                });
            }
            let mut bytes = Vec::new();
            io::stdin()
                .lock()
                .take(max as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| AppError::Io {
                    context: "reading standard input",
                    kind: error.kind(),
                })?;
            if bytes.len() > max {
                return Err(AppError::Usage {
                    message: format!("a request may contain at most {} KiB.", max / 1024),
                });
            }
            let text = String::from_utf8(bytes).map_err(|_| AppError::Usage {
                message: "standard input is not valid UTF-8.".to_owned(),
            })?;
            finish(text, max)
        }
    }
}

pub(crate) fn finish(text: String, max: usize) -> Result<String, AppError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AppError::Usage {
            message: "a request must contain non-empty text.".to_owned(),
        });
    }
    if trimmed.len() > max {
        return Err(AppError::Usage {
            message: format!("a request may contain at most {} KiB.", max / 1024),
        });
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_text_is_trimmed_and_bounded() {
        assert_eq!(finish("  hello  ".to_owned(), 10).unwrap(), "hello");
        assert_eq!(
            finish("   ".to_owned(), 10).unwrap_err().to_string(),
            "a request must contain non-empty text."
        );
        assert_eq!(
            finish("a".repeat(11), 10).unwrap_err().to_string(),
            "a request may contain at most 0 KiB."
        );
    }
}
