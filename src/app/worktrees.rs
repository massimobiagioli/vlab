//! Isolated worktree and execution environment ports (ADR-0050).
//!
//! The application decides which story runs; the infrastructure adapter owns
//! git and the filesystem. Neither port exposes provider content.

use std::path::PathBuf;

use crate::app::execution::ActionRunner;
use crate::domain::ids::{ProjectId, StoryId};

/// One prepared worktree for a story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    /// Absolute path of the worktree.
    pub path: PathBuf,
    /// Path relative to the workspace root, using `/` separators.
    pub relative_path: String,
    pub branch: String,
}

/// Failures preparing or committing a worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeError {
    /// The project has no checkout under `.workspace`.
    NotCloned,
    /// A stale worktree or branch could not be replaced.
    Occupied,
    /// `git` is missing.
    GitUnavailable,
    /// A git command failed or timed out.
    Git,
    /// The destination escaped the workspace.
    Confined,
    /// A filesystem operation failed.
    Io,
}

impl WorktreeError {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotCloned => "worktree_not_cloned",
            Self::Occupied => "worktree_occupied",
            Self::GitUnavailable => "worktree_git_unavailable",
            Self::Git => "worktree_git",
            Self::Confined => "worktree_confined",
            Self::Io => "worktree_io",
        }
    }
}

/// Prepares worktrees and their action runners.
pub trait WorkflowEnvironment {
    fn prepare(&self, project: &ProjectId, story: &StoryId) -> Result<Worktree, WorktreeError>;

    fn actions(&self, worktree: &Worktree) -> Box<dyn ActionRunner + '_>;
}
