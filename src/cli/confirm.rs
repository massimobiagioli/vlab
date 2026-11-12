//! Bound confirmation for destructive commands (A-08, WP-4-T5).

use crate::app::AppError;
use crate::app::agents::wizard::Prompter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmationRequest {
    /// The command flag, used in messages.
    pub command: &'static str,
    /// A concrete summary of the action and its affected entities.
    pub summary: String,
    /// `--yes` was supplied.
    pub assume_yes: bool,
}

/// Returns `Ok(())` only when the user explicitly confirms or supplied
/// `--yes`. Declining or EOF cancels with exit 5; a non-interactive stdin
/// without `--yes` fails closed with exit 4.
pub fn confirm(
    prompter: &mut impl Prompter,
    request: &ConfirmationRequest,
) -> Result<(), AppError> {
    if request.assume_yes {
        return Ok(());
    }
    if !prompter.is_interactive() {
        return Err(AppError::ConfirmationRequired {
            command: request.command,
        });
    }
    if prompter.confirm(&format!("{}\nProceed? [y/N]", request.summary))? {
        Ok(())
    } else {
        Err(AppError::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scripted {
        interactive: bool,
        answer: Option<bool>,
    }

    impl Prompter for Scripted {
        fn is_interactive(&self) -> bool {
            self.interactive
        }
        fn ask(&mut self, _: &crate::app::agents::wizard::Prompt<'_>) -> Result<String, AppError> {
            unreachable!()
        }
        fn confirm(&mut self, _: &str) -> Result<bool, AppError> {
            self.answer.ok_or(AppError::Cancelled)
        }
    }

    fn request(assume_yes: bool) -> ConfirmationRequest {
        ConfirmationRequest {
            command: "--delete-lab",
            summary: "Delete laboratory a3f91c07.".to_owned(),
            assume_yes,
        }
    }

    #[test]
    fn yes_bypasses_the_prompt() {
        let mut prompter = Scripted {
            interactive: false,
            answer: None,
        };
        assert!(confirm(&mut prompter, &request(true)).is_ok());
    }

    #[test]
    fn non_interactive_without_yes_fails_closed() {
        let mut prompter = Scripted {
            interactive: false,
            answer: None,
        };
        let error = confirm(&mut prompter, &request(false)).unwrap_err();
        assert!(matches!(error, AppError::ConfirmationRequired { .. }));
    }

    #[test]
    fn declining_cancels_and_accepting_proceeds() {
        let mut no = Scripted {
            interactive: true,
            answer: Some(false),
        };
        assert!(matches!(
            confirm(&mut no, &request(false)),
            Err(AppError::Cancelled)
        ));
        let mut yes = Scripted {
            interactive: true,
            answer: Some(true),
        };
        assert!(confirm(&mut yes, &request(false)).is_ok());
    }
}
