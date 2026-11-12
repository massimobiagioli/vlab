//! Exit codes (specification §14) and the `Error:`/`Hint:` contract.

use crate::app::AppError;
use crate::cli::render::sanitize;

pub const SUCCESS: u8 = 0;
pub const FAILURE: u8 = 1;
pub const INVALID_INPUT: u8 = 2;
pub const MISSING: u8 = 3;
pub const PRECONDITION: u8 = 4;
pub const CANCELLED: u8 = 5;
pub const INTERRUPTED: u8 = 130;

pub fn exit_code(error: &AppError) -> u8 {
    match error {
        AppError::Usage { .. } | AppError::Configuration { .. } => INVALID_INPUT,
        AppError::NotFound { .. } | AppError::NoActiveLab | AppError::NoActiveProject => MISSING,
        AppError::Validation { .. }
        | AppError::Precondition { .. }
        | AppError::ConfirmationRequired { .. } => PRECONDITION,
        AppError::Cancelled => CANCELLED,
        AppError::Interrupted => INTERRUPTED,
        AppError::Provider(_)
        | AppError::Storage { .. }
        | AppError::Workspace { .. }
        | AppError::Io { .. }
        | AppError::Internal => FAILURE,
    }
}

/// Renders an error for stderr, sanitized and newline-terminated.
///
/// The missing-context error keeps the exact wording of specification §3.4,
/// whose second line has no `Hint:` prefix.
pub fn render(error: &AppError) -> String {
    let problem = sanitize(&error.to_string());
    let hint = sanitize(&error.hint());
    match error {
        AppError::NoActiveLab | AppError::NoActiveProject => {
            format!("Error: {problem}\n{hint}\n")
        }
        _ => format!("Error: {problem}\nHint: {hint}\n"),
    }
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    use super::*;
    use crate::app::{EntityKind, ProviderFailure};

    fn text(s: &str) -> String {
        s.to_owned()
    }

    fn every_variant() -> Vec<(AppError, u8)> {
        vec![
            (
                AppError::Usage {
                    message: text("bad flag"),
                },
                2,
            ),
            (
                AppError::Configuration {
                    problem: text("p"),
                    hint: text("h"),
                },
                2,
            ),
            (
                AppError::NotFound {
                    entity: EntityKind::Laboratory,
                    id: text("a3f91c07"),
                },
                3,
            ),
            (AppError::NoActiveLab, 3),
            (AppError::NoActiveProject, 3),
            (
                AppError::Validation {
                    problem: text("p"),
                    hint: text("h"),
                },
                4,
            ),
            (
                AppError::Precondition {
                    problem: text("p"),
                    hint: text("h"),
                },
                4,
            ),
            (
                AppError::ConfirmationRequired {
                    command: "--delete-lab",
                },
                4,
            ),
            (AppError::Cancelled, 5),
            (AppError::Provider(ProviderFailure::Authentication), 1),
            (AppError::Provider(ProviderFailure::RateLimited), 1),
            (AppError::Provider(ProviderFailure::Transport), 1),
            (AppError::Provider(ProviderFailure::Timeout), 1),
            (AppError::Provider(ProviderFailure::InvalidOutput), 1),
            (AppError::Provider(ProviderFailure::Server), 1),
            (AppError::Provider(ProviderFailure::BudgetExceeded), 1),
            (AppError::Provider(ProviderFailure::ResponseTooLarge), 1),
            (AppError::Provider(ProviderFailure::Configuration), 1),
            (
                AppError::Storage {
                    problem: text("p"),
                    hint: text("h"),
                },
                1,
            ),
            (
                AppError::Workspace {
                    problem: text("p"),
                    hint: text("h"),
                },
                1,
            ),
            (
                AppError::Io {
                    context: "writing settings",
                    kind: ErrorKind::PermissionDenied,
                },
                1,
            ),
            (AppError::Interrupted, 130),
            (AppError::Internal, 1),
        ]
    }

    #[test]
    fn every_variant_maps_to_its_documented_exit_code() {
        for (error, code) in every_variant() {
            assert_eq!(exit_code(&error), code, "{error:?}");
        }
    }

    #[test]
    fn every_variant_renders_a_problem_and_an_action() {
        for (error, _) in every_variant() {
            let rendered = render(&error);
            let lines: Vec<&str> = rendered.lines().collect();
            assert_eq!(lines.len(), 2, "{rendered}");
            assert!(lines[0].len() > "Error: ".len(), "{rendered}");
            assert!(lines[0].starts_with("Error: "), "{rendered}");
            assert!(!lines[1].is_empty(), "{rendered}");
            assert!(!error.code().is_empty());
        }
    }

    #[test]
    fn unknown_entity_exits_3_with_an_actionable_hint() {
        let error = AppError::NotFound {
            entity: EntityKind::Agent,
            id: text("0b1c2d3e"),
        };
        assert_eq!(exit_code(&error), MISSING);
        assert_eq!(
            render(&error),
            "Error: no agent with identifier `0b1c2d3e` exists.\n\
             Hint: Run `vlab --list-agents` to see agents in the active laboratory.\n"
        );
    }

    #[test]
    fn missing_context_uses_the_specified_message() {
        assert_eq!(
            render(&AppError::NoActiveLab),
            "Error: no active laboratory is selected.\n\
             Run `vlab --list-labs`, then `vlab --open-lab <lab-id>`.\n"
        );
    }

    #[test]
    fn io_errors_keep_only_the_kind() {
        let error = AppError::Io {
            context: "writing settings",
            kind: ErrorKind::PermissionDenied,
        };
        assert_eq!(
            render(&error).lines().next(),
            Some("Error: writing settings failed: permission denied.")
        );
    }

    #[test]
    fn rendered_errors_are_sanitized() {
        let error = AppError::Usage {
            message: text("bad \u{1b}[31mvalue\u{7}"),
        };
        assert!(render(&error).starts_with("Error: bad value\n"));
    }
}
