//! Deterministic provider double for tests (ADR-0026).

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use crate::domain::budget::CancelToken;
use crate::domain::provider::{ModelProvider, ProviderError, ProviderRequest, ProviderResponse};

#[derive(Debug, Default)]
pub struct ScriptedProvider {
    outcomes: Mutex<VecDeque<Result<ProviderResponse, ProviderError>>>,
    requests: Mutex<Vec<ProviderRequest>>,
    cancel: Option<CancelToken>,
}

impl ScriptedProvider {
    pub fn new(outcomes: Vec<Result<ProviderResponse, ProviderError>>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into()),
            ..Self::default()
        }
    }

    pub fn with_cancel(cancel: CancelToken) -> Self {
        Self {
            cancel: Some(cancel),
            ..Self::default()
        }
    }

    pub fn push(&self, outcome: Result<ProviderResponse, ProviderError>) {
        lock(&self.outcomes).push_back(outcome);
    }

    pub fn requests(&self) -> Vec<ProviderRequest> {
        lock(&self.requests).clone()
    }

    pub fn remaining(&self) -> usize {
        lock(&self.outcomes).len()
    }
}

impl ModelProvider for ScriptedProvider {
    fn complete(&self, request: &ProviderRequest) -> Result<ProviderResponse, ProviderError> {
        if self.cancel.as_ref().is_some_and(CancelToken::is_cancelled) {
            return Err(ProviderError::Cancelled);
        }
        lock(&self.requests).push(request.clone());
        lock(&self.outcomes)
            .pop_front()
            .unwrap_or(Err(ProviderError::InvalidResponse))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
