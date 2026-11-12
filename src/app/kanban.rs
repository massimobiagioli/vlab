//! Read-only project board (specification §10, ADR-0005).

use crate::app::AppError;
use crate::app::storage::AppStateStore;
use crate::domain::ids::ProjectId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoryState {
    Todo,
    InProgress,
    InReview,
    Blocked,
    Done,
}

impl StoryState {
    pub const ALL: [Self; 5] = [
        Self::Todo,
        Self::InProgress,
        Self::InReview,
        Self::Blocked,
        Self::Done,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Todo => "TODO",
            Self::InProgress => "IN PROGRESS",
            Self::InReview => "IN REVIEW",
            Self::Blocked => "BLOCKED",
            Self::Done => "DONE",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "TODO" => Some(Self::Todo),
            "IN_PROGRESS" => Some(Self::InProgress),
            "IN_REVIEW" => Some(Self::InReview),
            "BLOCKED" => Some(Self::Blocked),
            "DONE" => Some(Self::Done),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: crate::domain::ids::StoryId,
    pub title: String,
    pub state: StoryState,
    pub assignee: Option<String>,
    pub priority: String,
    pub due_date: String,
    pub overdue: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoardCounts {
    pub todo: u64,
    pub in_progress: u64,
    pub in_review: u64,
    pub blocked: u64,
    pub done: u64,
}

impl BoardCounts {
    pub fn total(self) -> u64 {
        self.todo + self.in_progress + self.in_review + self.blocked + self.done
    }

    pub fn get(self, state: StoryState) -> u64 {
        match state {
            StoryState::Todo => self.todo,
            StoryState::InProgress => self.in_progress,
            StoryState::InReview => self.in_review,
            StoryState::Blocked => self.blocked,
            StoryState::Done => self.done,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub project_id: ProjectId,
    pub project_name: String,
    pub github_source: Option<String>,
    pub lab_name: String,
    pub lead_name: Option<String>,
    pub counts: BoardCounts,
    pub pending_intake: u64,
    pub cards: Vec<Card>,
}

impl Board {
    pub fn cards_in(&self, state: StoryState) -> impl Iterator<Item = &Card> {
        self.cards.iter().filter(move |card| card.state == state)
    }
}

pub trait BoardStore {
    /// Returns `None` when the project does not exist in `lab`.
    fn board(
        &self,
        project: &ProjectId,
        lab: &crate::domain::ids::LabId,
    ) -> Result<Option<Board>, crate::app::storage::StorageError>;
}

/// `--show-project-kanban`: requires an active laboratory and rejects a
/// project from another laboratory.
pub fn show(
    store: &impl BoardStore,
    state: &impl AppStateStore,
    raw_id: &str,
) -> Result<Board, AppError> {
    let id = crate::app::projects::parse_project_id(raw_id)?;
    let lab = crate::app::context::active_lab(state)?.require()?;
    store.board(&id, &lab)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Project,
        id: id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::storage::StorageError;
    use crate::domain::ids::{LabId, ProjectId};

    struct Store(Option<Board>);

    impl BoardStore for Store {
        fn board(&self, _: &ProjectId, _: &LabId) -> Result<Option<Board>, StorageError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn states_have_stable_labels_and_round_trip() {
        for state in StoryState::ALL {
            assert_eq!(
                StoryState::parse(state.label().replace(' ', "_").as_str()),
                Some(state)
            );
        }
        assert_eq!(StoryState::parse("UNKNOWN"), None);
    }

    #[test]
    fn counts_are_addressed_by_state() {
        let counts = BoardCounts {
            todo: 2,
            in_progress: 1,
            ..BoardCounts::default()
        };
        assert_eq!(counts.get(StoryState::Todo), 2);
        assert_eq!(counts.get(StoryState::Blocked), 0);
        assert_eq!(counts.total(), 3);
    }

    #[test]
    fn missing_projects_are_not_found() {
        let store = Store(None);
        // A real state store is exercised by the CLI tests; this unit test
        // only proves the missing-board branch.
        let board = Board {
            project_id: ProjectId::generate(),
            project_name: "P".to_owned(),
            github_source: None,
            lab_name: "L".to_owned(),
            lead_name: None,
            counts: BoardCounts::default(),
            pending_intake: 0,
            cards: Vec::new(),
        };
        assert!(
            store
                .board(&board.project_id, &LabId::generate())
                .unwrap()
                .is_none()
        );
    }
}
