//! The bounded model-provider port (ADR-0026).
//!
//! Domain code sees typed requests and errors only; no HTTP type leaks into
//! this module. Debug output deliberately omits request and response content.

use std::fmt;
use std::time::Duration;

/// One logical completion request.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderRequest {
    pub system: String,
    pub user: String,
    pub model: String,
    pub max_output_tokens: u32,
    pub json: bool,
}

impl fmt::Debug for ProviderRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderRequest")
            .field("system_len", &self.system.len())
            .field("user_len", &self.user.len())
            .field("model", &self.model)
            .field("max_output_tokens", &self.max_output_tokens)
            .field("json", &self.json)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<Usage>,
    pub finish_reason: Option<String>,
    pub request_id: Option<String>,
}

impl fmt::Debug for ProviderResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderResponse")
            .field("content_len", &self.content.len())
            .field("model", &self.model)
            .field("usage", &self.usage)
            .field("finish_reason", &self.finish_reason)
            .field("request_id", &self.request_id)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("the provider rejected the credentials.")]
    Authentication,
    #[error("the provider rate limit was reached.")]
    RateLimited { retry_after: Option<Duration> },
    #[error("the provider did not answer within the time limit.")]
    Timeout,
    #[error("the provider could not be reached.")]
    Transport,
    #[error("the provider returned server error {status}.")]
    Server {
        status: u16,
        request_id: Option<String>,
    },
    #[error("the provider returned a malformed response.")]
    InvalidResponse,
    #[error("the provider response exceeded the size limit.")]
    ResponseTooLarge,
    #[error("the operation was cancelled.")]
    Cancelled,
    #[error("the provider configuration is invalid.")]
    Configuration,
    #[error("the execution budget was exhausted.")]
    BudgetExceeded,
}

impl fmt::Debug for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl ProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Authentication => "provider_authentication",
            Self::RateLimited { .. } => "provider_rate_limited",
            Self::Timeout => "provider_timeout",
            Self::Transport => "provider_transport",
            Self::Server { .. } => "provider_server",
            Self::InvalidResponse => "provider_invalid_response",
            Self::ResponseTooLarge => "provider_response_too_large",
            Self::Cancelled => "cancelled",
            Self::Configuration => "provider_configuration",
            Self::BudgetExceeded => "budget_exceeded",
        }
    }

    /// Safe to retry without risking duplicate side effects. The operation is
    /// read-only from the application's point of view.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited { .. } | Self::Timeout | Self::Transport => true,
            Self::Server { status, .. } => *status >= 500,
            _ => false,
        }
    }
}

/// One bounded provider operation. Implementations must not expose HTTP types.
pub trait ModelProvider {
    fn complete(&self, request: &ProviderRequest) -> Result<ProviderResponse, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "sk-vlab-SECRET-MARKER";

    #[test]
    fn debug_output_never_contains_request_content() {
        let request = ProviderRequest {
            system: format!("system {SECRET}"),
            user: format!("user {SECRET}"),
            model: "deepseek-chat".to_owned(),
            max_output_tokens: 10,
            json: true,
        };
        let rendered = format!("{request:?}");
        assert!(!rendered.contains(SECRET), "{rendered}");
        assert!(rendered.contains("system_len"));
    }

    #[test]
    fn response_debug_never_contains_content() {
        let response = ProviderResponse {
            content: format!("answer {SECRET}"),
            model: "m".to_owned(),
            usage: None,
            finish_reason: None,
            request_id: None,
        };
        let rendered = format!("{response:?}");
        assert!(!rendered.contains(SECRET), "{rendered}");
        assert!(rendered.contains("content_len"));
    }

    #[test]
    fn retry_classes_are_narrow() {
        assert!(ProviderError::RateLimited { retry_after: None }.is_retryable());
        assert!(ProviderError::Timeout.is_retryable());
        assert!(ProviderError::Transport.is_retryable());
        assert!(
            ProviderError::Server {
                status: 500,
                request_id: None
            }
            .is_retryable()
        );
        assert!(
            !ProviderError::Server {
                status: 400,
                request_id: None
            }
            .is_retryable()
        );
        for error in [
            ProviderError::Authentication,
            ProviderError::InvalidResponse,
            ProviderError::ResponseTooLarge,
            ProviderError::Cancelled,
            ProviderError::Configuration,
            ProviderError::BudgetExceeded,
        ] {
            assert!(!error.is_retryable(), "{error:?}");
        }
    }
}
