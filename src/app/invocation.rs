//! Shared provider invocation service (WP-6-T1).
//!
//! Planning and questions use the same retry, budget, cancellation, event,
//! and interaction-metadata path. Interactions store no content.

use std::time::Instant;

use crate::app::provider_retry;
use crate::app::storage::StorageError;
use crate::domain::budget::Budget;
use crate::domain::clock::Clock;
use crate::domain::ids::AgentId;
use crate::domain::provider::{ModelProvider, ProviderError, ProviderRequest, ProviderResponse};
use crate::obs::events::{AuditEvent, EntityRef, EntityType, EventKind, Outcome};
use crate::obs::{ActionId, RunId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationKind {
    Planning,
    Question,
    /// One bounded simulated work or review call (ADR-0048).
    Workflow,
}

impl InvocationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Question => "question",
            Self::Workflow => "workflow",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interaction {
    pub run_id: RunId,
    pub agent_id: AgentId,
    pub kind: InvocationKind,
    pub provider: String,
    pub model: String,
    pub outcome: Outcome,
    pub error_code: Option<&'static str>,
    pub duration_ms: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

pub trait InvocationStore {
    fn record_interaction(&self, interaction: &Interaction) -> Result<(), StorageError>;
    fn record_event(&self, event: &AuditEvent) -> Result<(), StorageError>;
}

/// Runs one logical provider invocation through retry and budget, recording
/// attempt events and exactly one interaction row.
#[allow(clippy::too_many_arguments)]
pub fn invoke(
    store: &impl InvocationStore,
    agent_id: &AgentId,
    kind: InvocationKind,
    provider_name: &str,
    provider: &impl ModelProvider,
    request: &ProviderRequest,
    budget: &mut Budget,
    clock: &impl Clock,
    run_id: RunId,
) -> Result<ProviderResponse, ProviderError> {
    let started = Instant::now();
    let result = provider_retry::invoke(provider, request, budget, clock, |attempt, error| {
        let outcome = if error.is_some() {
            Outcome::Failure
        } else {
            Outcome::Success
        };
        let mut event = AuditEvent::new(
            run_id,
            ActionId::generate(),
            EventKind::ProviderAttempt,
            outcome,
        )
        .attempt(attempt);
        if let Some(entity) = EntityRef::new(EntityType::Agent, agent_id.as_str()) {
            event = event.entity(entity);
        }
        if let Some(error) = error {
            event = event.error_code(error.code());
        }
        let _ = store.record_event(&event);
    });
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let (outcome, error_code, usage) = match &result {
        Ok(response) => (Outcome::Success, None, response.usage),
        Err(ProviderError::Cancelled) => (Outcome::Cancelled, Some("cancelled"), None),
        Err(error) => (Outcome::Failure, Some(error.code()), None),
    };
    let interaction = Interaction {
        run_id,
        agent_id: agent_id.clone(),
        kind,
        provider: provider_name.to_owned(),
        model: request.model.clone(),
        outcome,
        error_code,
        duration_ms,
        input_tokens: usage.map(|usage| usage.input_tokens),
        output_tokens: usage.map(|usage| usage.output_tokens),
    };
    let _ = store.record_interaction(&interaction);
    result
}
