//! The single error type that crosses the application boundary.
//!
//! `Display` is the problem statement and [`AppError::hint`] the next useful
//! action. Neither may contain request text, provider bodies, file contents,
//! or credentials: variants carry only typed data or messages composed by
//! application code. The CLI maps each variant to an exit code.

use std::fmt;
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Invalid arguments or input.
    #[error("{message}")]
    Usage { message: String },

    /// Missing, malformed, or unusable configuration or environment.
    #[error("{problem}")]
    Configuration { problem: String, hint: String },

    /// A well-formed identifier that resolves to nothing.
    #[error("no {entity} with identifier `{id}` exists.")]
    NotFound { entity: EntityKind, id: String },

    /// A command requiring laboratory context ran without one
    /// (specification §3.4).
    #[error("no active laboratory is selected.")]
    NoActiveLab,

    /// Local validation of an entity or of model output failed.
    #[error("{problem}")]
    Validation { problem: String, hint: String },

    /// The operation is not allowed in the current state.
    #[error("{problem}")]
    Precondition { problem: String, hint: String },

    /// A destructive command needs confirmation but cannot prompt.
    #[error("`{command}` requires confirmation, but standard input is not a terminal.")]
    ConfirmationRequired { command: &'static str },

    /// The user declined or aborted.
    #[error("the operation was cancelled; no changes were made.")]
    Cancelled,

    /// A classified provider failure.
    #[error("{0}")]
    Provider(ProviderFailure),

    /// The local database failed or is unusable.
    #[error("{problem}")]
    Storage { problem: String, hint: String },

    /// A local I/O operation failed. Only the error kind is kept, never the
    /// underlying message, which may embed paths or data.
    #[error("{context} failed: {kind}.")]
    Io {
        context: &'static str,
        kind: io::ErrorKind,
    },

    /// Execution was interrupted by a signal.
    #[error("execution was interrupted.")]
    Interrupted,

    /// A violated internal invariant or a caught panic.
    #[error("an internal error occurred.")]
    Internal,
}

impl AppError {
    /// The next useful action for the user.
    pub fn hint(&self) -> String {
        match self {
            Self::Usage { .. } => "Run `vlab --help` to see the available commands.".to_owned(),
            Self::Configuration { hint, .. }
            | Self::Validation { hint, .. }
            | Self::Precondition { hint, .. }
            | Self::Storage { hint, .. } => hint.clone(),
            Self::NotFound { entity, .. } => entity.listing_hint().to_owned(),
            Self::NoActiveLab => {
                "Run `vlab --list-labs`, then `vlab --open-lab <lab-id>`.".to_owned()
            }
            Self::ConfirmationRequired { command } => {
                format!("Run `{command}` from a terminal, or add `--yes` to confirm explicitly.")
            }
            Self::Cancelled => "Run the command again when you are ready.".to_owned(),
            Self::Provider(failure) => failure.hint().to_owned(),
            Self::Io { .. } => {
                "Check that the path exists, is writable, and has free space, then retry."
                    .to_owned()
            }
            Self::Interrupted => "Run the command again; completed steps were saved.".to_owned(),
            Self::Internal => {
                "This is a bug in vlab. Re-run with RUST_BACKTRACE=1 and report the output."
                    .to_owned()
            }
        }
    }

    /// Stable machine-readable code, used for audit events.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Usage { .. } => "usage",
            Self::Configuration { .. } => "configuration",
            Self::NotFound { .. } => "not_found",
            Self::NoActiveLab => "no_active_lab",
            Self::Validation { .. } => "validation",
            Self::Precondition { .. } => "precondition",
            Self::ConfirmationRequired { .. } => "confirmation_required",
            Self::Cancelled => "cancelled",
            Self::Provider(failure) => failure.code(),
            Self::Storage { .. } => "storage",
            Self::Io { .. } => "io",
            Self::Interrupted => "interrupted",
            Self::Internal => "internal",
        }
    }
}

/// Persistent entity kinds that can be looked up by identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Laboratory,
    Agent,
    Project,
    Request,
    Story,
    Memory,
}

impl EntityKind {
    fn listing_hint(self) -> &'static str {
        match self {
            Self::Laboratory => "Run `vlab --list-labs` to see available laboratories.",
            Self::Agent => "Run `vlab --list-agents` to see agents in the active laboratory.",
            Self::Project => "Run `vlab --list-projects` to see projects in the active laboratory.",
            Self::Memory => "Run `vlab --list-memories` with a scope selector to see memories.",
            Self::Request | Self::Story => {
                "Run `vlab --show-project-kanban <project-id>` to see the project's work."
            }
        }
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Laboratory => "laboratory",
            Self::Agent => "agent",
            Self::Project => "project",
            Self::Request => "request",
            Self::Story => "story",
            Self::Memory => "memory",
        })
    }
}

/// Provider failures distinguished by specification §12.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderFailure {
    #[error("the provider rejected the credentials.")]
    Authentication,
    #[error("the provider rate limit was reached.")]
    RateLimited,
    #[error("the provider could not be reached.")]
    Transport,
    #[error("the provider did not answer within the time limit.")]
    Timeout,
    #[error("the provider returned output that failed local validation.")]
    InvalidOutput,
    #[error("the provider returned a server error.")]
    Server,
    #[error("the execution budget was exhausted.")]
    BudgetExceeded,
    #[error("the provider response exceeded the size limit.")]
    ResponseTooLarge,
    #[error("the provider configuration is invalid.")]
    Configuration,
}

impl ProviderFailure {
    pub fn from_provider(error: &crate::domain::provider::ProviderError) -> Self {
        use crate::domain::provider::ProviderError as E;
        match error {
            E::Authentication => Self::Authentication,
            E::RateLimited { .. } => Self::RateLimited,
            E::Timeout => Self::Timeout,
            E::Transport => Self::Transport,
            E::Server { .. } => Self::Server,
            E::InvalidResponse => Self::InvalidOutput,
            E::ResponseTooLarge => Self::ResponseTooLarge,
            E::Cancelled => Self::Transport,
            E::Configuration => Self::Configuration,
            E::BudgetExceeded => Self::BudgetExceeded,
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Authentication => "Check that DEEPSEEK_API_KEY holds a valid key.",
            Self::RateLimited => "Wait a moment, then submit the command again.",
            Self::Transport => "Check your network connection, then retry.",
            Self::Timeout => "Retry later; the provider may be overloaded.",
            Self::InvalidOutput => "Submit a revised request; the previous record is kept.",
            Self::Server => "Retry later; the provider reported a server error.",
            Self::BudgetExceeded => {
                "Retry later; the operation exceeded its time or attempt budget."
            }
            Self::ResponseTooLarge => "Retry later; the provider response exceeded the size limit.",
            Self::Configuration => "Check the provider configuration and DEEPSEEK_API_KEY.",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Authentication => "provider_authentication",
            Self::RateLimited => "provider_rate_limited",
            Self::Transport => "provider_transport",
            Self::Timeout => "provider_timeout",
            Self::InvalidOutput => "provider_invalid_output",
            Self::Server => "provider_server",
            Self::BudgetExceeded => "budget_exceeded",
            Self::ResponseTooLarge => "provider_response_too_large",
            Self::Configuration => "provider_configuration",
        }
    }
}
