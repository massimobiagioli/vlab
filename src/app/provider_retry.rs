//! Bounded provider retries (ADR-0026).
//!
//! Only transient, read-only failures are retried. Every attempt consumes the
//! shared budget; cancellation or an exhausted deadline stops immediately.

use std::time::{Duration, Instant};

use crate::domain::budget::{Budget, BudgetError};
use crate::domain::clock::Clock;
use crate::domain::provider::{ModelProvider, ProviderError, ProviderRequest, ProviderResponse};

/// Calls the provider through the retry policy. `on_attempt` receives the
/// attempt number and the failure (or `None` on success) for audit events.
pub fn invoke(
    provider: &impl ModelProvider,
    request: &ProviderRequest,
    budget: &mut Budget,
    clock: &impl Clock,
    mut on_attempt: impl FnMut(u32, Option<&ProviderError>),
) -> Result<ProviderResponse, ProviderError> {
    let limits = budget.limits();
    let mut last_error = None;
    loop {
        if budget.cancelled() {
            return Err(ProviderError::Cancelled);
        }
        let attempt = match budget.begin_attempt() {
            Ok(attempt) => attempt,
            Err(BudgetError::Cancelled) => return Err(ProviderError::Cancelled),
            Err(BudgetError::Exhausted) => {
                return Err(last_error.unwrap_or(ProviderError::BudgetExceeded));
            }
        };
        if attempt == 1 {
            budget.mark_invocation();
        }
        let started = Instant::now();
        let outcome = provider.complete(request);
        let _ = budget.consume(started.elapsed());
        match outcome {
            Ok(response) => {
                on_attempt(attempt, None);
                return Ok(response);
            }
            Err(error) => {
                on_attempt(attempt, Some(&error));
                if !error.is_retryable() {
                    return Err(error);
                }
                last_error = Some(error.clone());
                if attempt >= limits.max_attempts {
                    return Err(error);
                }
                let backoff = retry_after(&error)
                    .unwrap_or_else(|| backoff(limits.backoff_base, limits.backoff_cap, attempt));
                if budget.cancelled() {
                    return Err(ProviderError::Cancelled);
                }
                if !budget.consume(backoff) {
                    return Err(ProviderError::BudgetExceeded);
                }
                clock.sleep(backoff);
            }
        }
    }
}

fn retry_after(error: &ProviderError) -> Option<Duration> {
    match error {
        ProviderError::RateLimited { retry_after } => *retry_after,
        _ => None,
    }
}

fn backoff(base: Duration, cap: Duration, attempt: u32) -> Duration {
    let factor = 1u32
        .checked_shl(attempt.saturating_sub(1))
        .unwrap_or(u32::MAX);
    base.saturating_mul(factor).min(cap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::budget::{CancelToken, ExecutionLimits};
    use crate::domain::clock::TestClock;
    use crate::infra::provider::scripted::ScriptedProvider;

    fn response() -> ProviderResponse {
        ProviderResponse {
            content: "{}".to_owned(),
            model: "m".to_owned(),
            usage: None,
            finish_reason: None,
            request_id: None,
        }
    }

    fn request() -> ProviderRequest {
        ProviderRequest {
            system: "s".to_owned(),
            user: "u".to_owned(),
            model: "m".to_owned(),
            max_output_tokens: 10,
            json: true,
        }
    }

    #[test]
    fn retryable_failures_produce_three_attempts_and_backoff() {
        let provider = ScriptedProvider::new(vec![
            Err(ProviderError::RateLimited { retry_after: None }),
            Err(ProviderError::RateLimited { retry_after: None }),
            Ok(response()),
        ]);
        let clock = TestClock::default();
        let mut budget = Budget::new(ExecutionLimits::default(), CancelToken::default());
        let mut attempts = Vec::new();
        let result = invoke(&provider, &request(), &mut budget, &clock, |n, e| {
            attempts.push((n, e.map(ProviderError::code)));
        });
        assert!(result.is_ok());
        assert_eq!(attempts.len(), 3);
        assert_eq!(
            clock.sleeps(),
            [Duration::from_millis(500), Duration::from_secs(1)]
        );
        assert_eq!(provider.requests().len(), 3);
    }

    #[test]
    fn non_retryable_failure_is_dispatched_once() {
        let provider = ScriptedProvider::new(vec![Err(ProviderError::Authentication)]);
        let clock = TestClock::default();
        let mut budget = Budget::new(ExecutionLimits::default(), CancelToken::default());
        assert!(matches!(
            invoke(&provider, &request(), &mut budget, &clock, |_, _| {}),
            Err(ProviderError::Authentication)
        ));
        assert_eq!(provider.requests().len(), 1);
    }

    #[test]
    fn cancellation_prevents_dispatch() {
        let cancel = CancelToken::default();
        cancel.cancel();
        let provider = ScriptedProvider::with_cancel(cancel.clone());
        let clock = TestClock::default();
        let mut budget = Budget::new(ExecutionLimits::default(), cancel);
        assert!(matches!(
            invoke(&provider, &request(), &mut budget, &clock, |_, _| {}),
            Err(ProviderError::Cancelled)
        ));
        assert!(provider.requests().is_empty());
    }

    #[test]
    fn retry_after_takes_precedence_and_deadline_stops_dispatch() {
        let provider = ScriptedProvider::new(vec![Err(ProviderError::RateLimited {
            retry_after: Some(Duration::from_secs(10)),
        })]);
        let clock = TestClock::default();
        let mut budget = Budget::new(
            ExecutionLimits {
                total_deadline: Duration::from_secs(5),
                ..ExecutionLimits::default()
            },
            CancelToken::default(),
        );
        assert!(matches!(
            invoke(&provider, &request(), &mut budget, &clock, |_, _| {}),
            Err(ProviderError::BudgetExceeded)
        ));
        assert_eq!(provider.requests().len(), 1);
        assert!(clock.sleeps().is_empty());
    }
}
