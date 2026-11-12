//! Tech Lead planning (WP-5): prompt, schema, rules, and orchestration.

pub mod prompt;
pub mod rules;
pub mod schema;

use crate::app::storage::StorageError;
use crate::domain::ids::{ProjectId, RequestId, StoryId};
use crate::obs::events::AuditEvent;

/// A story ready for persistence after local validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedStoryRow {
    pub id: StoryId,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub priority: String,
    pub complexity: String,
    pub required_skills: Vec<String>,
    pub assigned_agent_id: Option<String>,
    pub assignment_rationale: String,
    pub due_date: String,
}

/// Persistence port for request and story lifecycle.
pub trait PlanningStore {
    fn insert_request(
        &self,
        id: &RequestId,
        project: &ProjectId,
        content: &str,
    ) -> Result<(), StorageError>;

    fn request_created_at(&self, id: &RequestId) -> Result<Option<String>, StorageError>;

    fn mark_planning(&self, id: &RequestId) -> Result<(), StorageError>;

    fn finish_planned(
        &self,
        id: &RequestId,
        project: &ProjectId,
        stories: &[PlannedStoryRow],
    ) -> Result<(), StorageError>;

    fn finish_clarification(
        &self,
        id: &RequestId,
        questions: &[String],
    ) -> Result<(), StorageError>;

    fn finish_failed(&self, id: &RequestId, code: &str, message: &str) -> Result<(), StorageError>;

    fn recover_stale_planning(&self, lease_seconds: i64) -> Result<u64, StorageError>;

    /// Persists a typed audit event. Best effort at the call site.
    fn record_event(&self, event: &AuditEvent) -> Result<(), StorageError>;
}
