//! Read-only laboratory metrics for the home dashboard (ADR-0051).

use crate::app::AppError;
use crate::app::kanban::BoardCounts;
use crate::app::storage::StorageError;
use crate::domain::ids::{LabId, ProjectId};

/// Card counts for one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMetrics {
    pub id: ProjectId,
    pub name: String,
    pub counts: BoardCounts,
}

impl ProjectMetrics {
    pub fn total(&self) -> u64 {
        self.counts.total()
    }

    /// TODO + IN_PROGRESS + IN_REVIEW + BLOCKED.
    pub fn open(&self) -> u64 {
        self.total().saturating_sub(self.counts.done)
    }

    pub fn closed(&self) -> u64 {
        self.counts.done
    }
}

/// Aggregate metrics for one laboratory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabMetrics {
    pub lab_id: LabId,
    pub lab_name: String,
    pub projects: Vec<ProjectMetrics>,
    pub pending_intake: u64,
}

impl LabMetrics {
    pub fn counts(&self) -> BoardCounts {
        let mut counts = BoardCounts::default();
        for project in &self.projects {
            counts.todo += project.counts.todo;
            counts.in_progress += project.counts.in_progress;
            counts.in_review += project.counts.in_review;
            counts.blocked += project.counts.blocked;
            counts.done += project.counts.done;
        }
        counts
    }

    pub fn open(&self) -> u64 {
        self.counts().total().saturating_sub(self.counts().done)
    }

    pub fn closed(&self) -> u64 {
        self.counts().done
    }

    pub fn total(&self) -> u64 {
        self.counts().total()
    }

    /// Completion percentage, or `None` when there are no stories.
    pub fn completion(&self) -> Option<u64> {
        let total = self.total();
        (total > 0).then(|| self.closed() * 100 / total)
    }
}

/// Loads the metrics for one laboratory.
pub fn show(store: &impl MetricsStore, lab: &LabId) -> Result<LabMetrics, AppError> {
    store.lab_metrics(lab).map_err(AppError::from)
}

/// Persistence port for the metrics aggregate.
pub trait MetricsStore {
    fn lab_metrics(&self, lab: &LabId) -> Result<LabMetrics, StorageError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(
        todo: u64,
        in_progress: u64,
        in_review: u64,
        blocked: u64,
        done: u64,
    ) -> ProjectMetrics {
        ProjectMetrics {
            id: ProjectId::parse("0b1c2d3e").unwrap(),
            name: "Portal".to_owned(),
            counts: BoardCounts {
                todo,
                in_progress,
                in_review,
                blocked,
                done,
            },
        }
    }

    #[test]
    fn open_closed_and_completion_follow_the_definitions() {
        let metrics = LabMetrics {
            lab_id: LabId::parse("a3f91c07").unwrap(),
            lab_name: "Payments".to_owned(),
            projects: vec![project(2, 1, 0, 1, 4)],
            pending_intake: 3,
        };
        assert_eq!(metrics.open(), 4);
        assert_eq!(metrics.closed(), 4);
        assert_eq!(metrics.total(), 8);
        assert_eq!(metrics.completion(), Some(50));
    }

    #[test]
    fn an_empty_laboratory_has_no_completion_percentage() {
        let metrics = LabMetrics {
            lab_id: LabId::parse("a3f91c07").unwrap(),
            lab_name: "Payments".to_owned(),
            projects: vec![project(0, 0, 0, 0, 0)],
            pending_intake: 0,
        };
        assert_eq!(metrics.total(), 0);
        assert_eq!(metrics.completion(), None);
    }
}
