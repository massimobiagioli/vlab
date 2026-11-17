//! Durable request queue (ADR-0061).
//!
//! Intake persists a request as `TODO` and returns. The interactive worker or
//! the hidden drain command selects the oldest queued request for the active
//! laboratory and runs the existing planning and workflow. The queue is the
//! existing request status; this module adds no storage of its own.

use crate::app::AppError;
use crate::app::storage::StorageError;
use crate::domain::ids::{LabId, ProjectId, RequestId};

/// One request waiting for the Tech Lead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedRequest {
    pub id: RequestId,
    pub project_id: ProjectId,
    pub lab_id: LabId,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

/// Read side of the durable queue.
pub trait QueueStore {
    /// The oldest `TODO` request in `lab`, or `None`.
    fn next_todo_request(&self, lab: &LabId) -> Result<Option<QueuedRequest>, StorageError>;

    /// All `TODO` and `PLANNING` requests in `lab`, oldest first.
    fn queued_requests(&self, lab: &LabId) -> Result<Vec<QueuedRequest>, StorageError>;
}

/// The oldest queued request for the active laboratory.
pub fn next(store: &impl QueueStore, lab: &LabId) -> Result<Option<QueuedRequest>, AppError> {
    Ok(store.next_todo_request(lab)?)
}

/// Every request currently in `TODO` or `PLANNING`, oldest first.
pub fn queued(store: &impl QueueStore, lab: &LabId) -> Result<Vec<QueuedRequest>, AppError> {
    Ok(store.queued_requests(lab)?)
}

/// Largest batch processed by the hidden headless drain.
pub const MAX_QUEUE_BATCH: usize = 8;
