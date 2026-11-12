//! Typed audit events (implementation plan §4.5).
//!
//! The API has no free-text field: every value is an enum, a number, or a
//! validated short token, so request text, prompts, responses, CVs, and
//! credentials cannot be recorded. WP-2 adds the SQLite sink that persists
//! events with a `created_at` timestamp.

use std::fmt;

use super::{ActionId, RunId};

/// Domain-relevant actions that are audited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    MigrationApplied,
    LabCreated,
    LabOpened,
    LabDeleted,
    AgentCreated,
    AgentArchived,
    ProjectCreated,
    ProjectDeleted,
    QuestionAnswered,
    MemoryCreated,
    MemoryDeleted,
    RequestAccepted,
    PlanningStarted,
    PlanningCompleted,
    ProviderAttempt,
    PlanningFailed,
    ConfirmationGranted,
    ConfirmationDeclined,
    BudgetExhausted,
    Cancelled,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MigrationApplied => "migration_applied",
            Self::LabCreated => "lab_created",
            Self::LabOpened => "lab_opened",
            Self::LabDeleted => "lab_deleted",
            Self::AgentCreated => "agent_created",
            Self::AgentArchived => "agent_archived",
            Self::ProjectCreated => "project_created",
            Self::ProjectDeleted => "project_deleted",
            Self::QuestionAnswered => "question_answered",
            Self::MemoryCreated => "memory_created",
            Self::MemoryDeleted => "memory_deleted",
            Self::RequestAccepted => "request_accepted",
            Self::PlanningStarted => "planning_started",
            Self::PlanningCompleted => "planning_completed",
            Self::ProviderAttempt => "provider_attempt",
            Self::PlanningFailed => "planning_failed",
            Self::ConfirmationGranted => "confirmation_granted",
            Self::ConfirmationDeclined => "confirmation_declined",
            Self::BudgetExhausted => "budget_exhausted",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Failure,
    Cancelled,
    Denied,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
            Self::Denied => "denied",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    Laboratory,
    Agent,
    Project,
    Request,
    Story,
    Memory,
}

impl EntityType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Laboratory => "laboratory",
            Self::Agent => "agent",
            Self::Project => "project",
            Self::Request => "request",
            Self::Story => "story",
            Self::Memory => "memory",
        }
    }
}

/// The entity an event concerns, with an identifier in the ADR-0004 format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRef {
    entity_type: EntityType,
    id: String,
}

impl EntityRef {
    /// Accepts only 8 lowercase hexadecimal characters.
    pub fn new(entity_type: EntityType, id: &str) -> Option<Self> {
        let well_formed = id.len() == 8
            && id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        well_formed.then(|| Self {
            entity_type,
            id: id.to_owned(),
        })
    }

    pub fn entity_type(&self) -> EntityType {
        self.entity_type
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    DeepSeek,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
        }
    }
}

/// A provider model identifier: 1–64 characters of `[A-Za-z0-9._:-]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelName(String);

impl ModelName {
    pub fn new(name: &str) -> Option<Self> {
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-');
        (!name.is_empty() && name.len() <= 64 && name.chars().all(allowed))
            .then(|| Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    pub run_id: RunId,
    pub action_id: ActionId,
    pub kind: EventKind,
    pub outcome: Outcome,
    pub entity: Option<EntityRef>,
    /// A stable code such as `AppError::code()`; never a message.
    pub error_code: Option<&'static str>,
    pub duration_ms: Option<u64>,
    pub attempt: Option<u32>,
    pub provider: Option<Provider>,
    pub model: Option<ModelName>,
    /// Schema version before `migration_applied`; otherwise `None`.
    pub schema_from: Option<i64>,
    /// Schema version after `migration_applied`; otherwise `None`.
    pub schema_to: Option<i64>,
}

impl AuditEvent {
    pub fn new(run_id: RunId, action_id: ActionId, kind: EventKind, outcome: Outcome) -> Self {
        Self {
            run_id,
            action_id,
            kind,
            outcome,
            entity: None,
            error_code: None,
            duration_ms: None,
            attempt: None,
            provider: None,
            model: None,
            schema_from: None,
            schema_to: None,
        }
    }

    pub fn entity(mut self, entity: EntityRef) -> Self {
        self.entity = Some(entity);
        self
    }

    pub fn error_code(mut self, code: &'static str) -> Self {
        self.error_code = Some(code);
        self
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn attempt(mut self, attempt: u32) -> Self {
        self.attempt = Some(attempt);
        self
    }

    pub fn provider(mut self, provider: Provider, model: Option<ModelName>) -> Self {
        self.provider = Some(provider);
        self.model = model;
        self
    }

    /// Records the schema versions of a migration. Numbers only, so no
    /// migration content can be captured.
    pub fn schema_versions(mut self, from: i64, to: i64) -> Self {
        self.schema_from = Some(from);
        self.schema_to = Some(to);
        self
    }
}

/// Destination for audit events.
pub trait EventSink {
    fn record(&mut self, event: &AuditEvent);
}

/// Emits events as `vlab::audit` diagnostics.
#[derive(Debug, Default)]
pub struct TracingSink;

impl EventSink for TracingSink {
    fn record(&mut self, event: &AuditEvent) {
        tracing::info!(
            target: "vlab::audit",
            run_id = %event.run_id,
            action_id = %event.action_id,
            kind = event.kind.as_str(),
            outcome = event.outcome.as_str(),
            entity_type = event.entity.as_ref().map(|e| e.entity_type.as_str()),
            entity_id = event.entity.as_ref().map(|e| e.id.as_str()),
            error_code = event.error_code,
            duration_ms = event.duration_ms,
            attempt = event.attempt,
            provider = event.provider.map(Provider::as_str),
            model = event.model.as_ref().map(ModelName::as_str),
            schema_from = event.schema_from,
            schema_to = event.schema_to,
        );
    }
}

/// Keeps events in memory; for tests.
#[derive(Debug, Default)]
pub struct MemorySink {
    pub events: Vec<AuditEvent>,
}

impl EventSink for MemorySink {
    fn record(&mut self, event: &AuditEvent) {
        self.events.push(event.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_ids_must_be_short_lowercase_hex() {
        assert!(EntityRef::new(EntityType::Laboratory, "a3f91c07").is_some());
        for bad in [
            "",
            "a3f91c0",
            "a3f91c070",
            "A3F91C07",
            "a3f91c0g",
            "../etc/x",
            "a3f9 c07",
        ] {
            assert!(
                EntityRef::new(EntityType::Laboratory, bad).is_none(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn model_names_reject_free_text() {
        assert!(ModelName::new("deepseek-chat").is_some());
        assert!(ModelName::new("vendor/model:v1.2").is_none());
        for bad in [
            "",
            "two words",
            "line\nbreak",
            "\u{1b}[31m",
            &"m".repeat(65),
        ] {
            assert!(ModelName::new(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn builder_records_typed_fields_only() {
        let run = RunId::generate();
        let action = ActionId::generate();
        let entity = EntityRef::new(EntityType::Project, "0b1c2d3e").unwrap();
        let event = AuditEvent::new(run, action, EventKind::PlanningFailed, Outcome::Failure)
            .entity(entity.clone())
            .error_code("provider_timeout")
            .duration_ms(1_500)
            .attempt(3)
            .provider(Provider::DeepSeek, ModelName::new("deepseek-chat"));

        let mut sink = MemorySink::default();
        sink.record(&event);
        TracingSink.record(&event);

        let recorded = &sink.events[0];
        assert_eq!(recorded.entity, Some(entity));
        assert_eq!(recorded.error_code, Some("provider_timeout"));
        assert_eq!(recorded.attempt, Some(3));
        assert_eq!(
            recorded.model.as_ref().map(ModelName::as_str),
            Some("deepseek-chat")
        );
    }

    #[test]
    fn run_and_action_ids_are_distinct_hex_tokens() {
        let (a, b) = (RunId::generate().to_string(), RunId::generate().to_string());
        assert_ne!(a, b);
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
    }
}
