//! Isolated worktree and execution environment ports (ADR-0050, ADR-0070).
//!
//! The application decides which story runs and when a DONE story is
//! integrated; the infrastructure adapter owns git and the filesystem. Neither
//! port exposes provider content.

use std::io;
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
    /// A develop integration or sandbox operation failed.
    Repository(RepositoryError),
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
            Self::Repository(error) => error.code(),
        }
    }
}

/// The result of integrating one story into `develop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevelopIntegration {
    /// The commit now on `develop` (the merge commit or the existing commit).
    pub merge_commit: String,
    /// Whether a remote push succeeded.
    pub pushed: bool,
    /// Whether the base checkout has an `origin` remote.
    pub remote: bool,
    /// Whether the story commit was already an ancestor of `develop`.
    pub already_integrated: bool,
}

/// A refreshed testable checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxCheckout {
    pub path: PathBuf,
}

/// Failures integrating a story or refreshing the sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryError {
    /// The project has no checkout under `.workspace`.
    NotCloned,
    /// `develop` does not exist and cannot be created.
    NoDevelop,
    /// A merge conflict or divergent remote branch needs a human.
    Conflict,
    /// `git` is missing.
    GitUnavailable,
    /// A git command timed out.
    Timeout,
    /// A git command failed.
    Git,
    /// A path escaped the workspace or sandbox root.
    Confined,
    /// A filesystem operation failed.
    Io(io::ErrorKind),
}

impl RepositoryError {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotCloned => "develop_not_cloned",
            Self::NoDevelop => "develop_missing",
            Self::Conflict => "develop_conflict",
            Self::GitUnavailable => "develop_git_unavailable",
            Self::Timeout => "develop_timeout",
            Self::Git => "develop_git",
            Self::Confined => "develop_confined",
            Self::Io(_) => "develop_io",
        }
    }
}

/// Integrates reviewed commits and materializes testable checkouts (ADR-0070).
pub trait ProjectRepository {
    /// Merges `commit` from `branch` into `develop` and pushes it when a remote
    /// exists. Repeated calls are idempotent.
    fn integrate_story(
        &self,
        project: &ProjectId,
        story: &StoryId,
        branch: &str,
        commit: &str,
    ) -> Result<DevelopIntegration, RepositoryError>;

    /// Clones or refreshes the sandbox checkout from `develop`.
    fn sync_sandbox(&self, project: &ProjectId) -> Result<SandboxCheckout, RepositoryError>;
}

/// Prepares worktrees, their action runners, and optional develop integration.
pub trait WorkflowEnvironment {
    fn prepare(&self, project: &ProjectId, story: &StoryId) -> Result<Worktree, WorktreeError>;

    fn actions(&self, worktree: &Worktree) -> Box<dyn ActionRunner + '_>;

    /// Integrates a DONE story. `None` means integration is not configured
    /// (for example in tests that only exercise execution).
    fn integrate_story(
        &self,
        _project: &ProjectId,
        _story: &StoryId,
        _branch: &str,
        _commit: &str,
    ) -> Result<Option<DevelopIntegration>, WorktreeError> {
        Ok(None)
    }

    /// Refreshes the project sandbox. `None` means integration is not
    /// configured.
    fn sync_sandbox(&self, _project: &ProjectId) -> Result<Option<SandboxCheckout>, WorktreeError> {
        Ok(None)
    }
}
