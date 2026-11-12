//! Execution limits, deadlines, counters, and cancellation (AD-8, §12).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Production defaults from the implementation plan Appendix D.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionLimits {
    pub max_attempts: u32,
    pub attempt_timeout: Duration,
    pub total_deadline: Duration,
    pub question_deadline: Duration,
    pub request_max_bytes: usize,
    pub question_max_bytes: usize,
    pub response_max_bytes: usize,
    pub max_stories: usize,
    pub max_output_tokens: u32,
    pub backoff_base: Duration,
    pub backoff_cap: Duration,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            attempt_timeout: Duration::from_secs(60),
            total_deadline: Duration::from_secs(240),
            question_deadline: Duration::from_secs(180),
            request_max_bytes: 64 * 1024,
            question_max_bytes: 8 * 1024,
            response_max_bytes: 256 * 1024,
            max_stories: 20,
            max_output_tokens: 4_096,
            backoff_base: Duration::from_millis(500),
            backoff_cap: Duration::from_secs(8),
        }
    }
}

/// Remaining time for one logical operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    remaining: Duration,
}

impl Deadline {
    pub fn new(total: Duration) -> Self {
        Self { remaining: total }
    }

    pub fn remaining(self) -> Duration {
        self.remaining
    }

    pub fn expired(self) -> bool {
        self.remaining.is_zero()
    }

    /// Returns false when the duration does not fit in the remaining budget.
    pub fn consume(&mut self, duration: Duration) -> bool {
        match self.remaining.checked_sub(duration) {
            Some(remaining) => {
                self.remaining = remaining;
                true
            }
            None => {
                self.remaining = Duration::ZERO;
                false
            }
        }
    }
}

/// A cooperative cancellation flag shared with the provider call.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    /// No attempt may be started.
    Exhausted,
    /// The caller cancelled the operation.
    Cancelled,
}

/// Tracks attempts and the total deadline for one logical operation.
#[derive(Debug)]
pub struct Budget {
    limits: ExecutionLimits,
    deadline: Deadline,
    attempts: u32,
    invocations: u32,
    cancel: CancelToken,
}

impl Budget {
    pub fn new(limits: ExecutionLimits, cancel: CancelToken) -> Self {
        Self {
            limits,
            deadline: Deadline::new(limits.total_deadline),
            attempts: 0,
            invocations: 0,
            cancel,
        }
    }

    pub fn limits(&self) -> ExecutionLimits {
        self.limits
    }

    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.remaining()
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn invocations(&self) -> u32 {
        self.invocations
    }

    /// Reserves one logical invocation and one HTTP attempt.
    pub fn begin_attempt(&mut self) -> Result<u32, BudgetError> {
        if self.cancel.is_cancelled() {
            return Err(BudgetError::Cancelled);
        }
        if self.deadline.expired() || self.attempts >= self.limits.max_attempts {
            return Err(BudgetError::Exhausted);
        }
        self.attempts += 1;
        Ok(self.attempts)
    }

    pub fn mark_invocation(&mut self) {
        self.invocations += 1;
    }

    /// Consumes elapsed or backoff time. Returns false when the deadline is
    /// exhausted, in which case no further attempt may start.
    pub fn consume(&mut self, duration: Duration) -> bool {
        self.deadline.consume(duration)
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> Budget {
        Budget::new(ExecutionLimits::default(), CancelToken::default())
    }

    #[test]
    fn deadlines_have_exact_boundaries() {
        let mut deadline = Deadline::new(Duration::from_secs(10));
        assert!(deadline.consume(Duration::from_secs(5)));
        assert_eq!(deadline.remaining(), Duration::from_secs(5));
        assert!(deadline.consume(Duration::from_secs(5)));
        assert!(deadline.expired());
        assert!(!deadline.consume(Duration::from_millis(1)));
        assert!(deadline.expired());
    }

    #[test]
    fn attempts_stop_at_the_limit() {
        let mut budget = budget();
        for expected in 1..=3 {
            assert_eq!(budget.begin_attempt(), Ok(expected));
        }
        assert_eq!(budget.begin_attempt(), Err(BudgetError::Exhausted));
    }

    #[test]
    fn zero_remaining_budget_prevents_dispatch() {
        let mut budget = budget();
        budget.consume(budget.remaining());
        assert_eq!(budget.begin_attempt(), Err(BudgetError::Exhausted));
    }

    #[test]
    fn cancellation_prevents_any_dispatch_even_with_budget() {
        let cancel = CancelToken::default();
        let mut budget = Budget::new(ExecutionLimits::default(), cancel.clone());
        cancel.cancel();
        assert_eq!(budget.begin_attempt(), Err(BudgetError::Cancelled));
    }

    #[test]
    fn backoff_cannot_exceed_the_total_deadline() {
        let mut budget = budget();
        budget.consume(budget.remaining());
        assert!(!budget.consume(Duration::from_millis(1)));
        assert_eq!(budget.remaining(), Duration::ZERO);
    }
}
