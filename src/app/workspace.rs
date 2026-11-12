//! Project checkout workspace (ADR-0032).
//!
//! The application decides *when* a project is cloned; the infrastructure
//! adapter decides *how*. No git output or credential crosses this boundary.

use std::io;
use std::path::{Path, PathBuf};

use crate::app::AppError;
use crate::domain::ids::ProjectId;
use crate::domain::project::GithubSource;

/// A local checkout for one project.
pub trait ProjectWorkspace {
    /// Clones `source` into a new directory for `id`.
    fn clone_project(
        &self,
        source: &GithubSource,
        id: &ProjectId,
    ) -> Result<PathBuf, WorkspaceError>;

    /// Removes a checkout after a failed project insert. Best effort.
    fn remove_project(&self, path: &Path);
}

/// Failures that the workspace adapter can report without exposing git output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceError {
    DestinationExists,
    GitUnavailable,
    Timeout,
    Failed,
    Io(io::ErrorKind),
}

impl WorkspaceError {
    pub fn into_app_error(self) -> AppError {
        match self {
            Self::DestinationExists => AppError::Precondition {
                problem: "the project workspace directory already exists.".to_owned(),
                hint: "Remove the existing directory under `.workspace/` and retry.".to_owned(),
            },
            Self::GitUnavailable => AppError::Configuration {
                problem: "the `git` executable is not available.".to_owned(),
                hint: "Install git and ensure it is on PATH, then retry.".to_owned(),
            },
            Self::Timeout => AppError::Workspace {
                problem: "cloning the project repository timed out.".to_owned(),
                hint: "Check the network connection and retry; the project was not created."
                    .to_owned(),
            },
            Self::Failed => AppError::Workspace {
                problem: "the project repository could not be cloned.".to_owned(),
                hint: "Check the repository URL, the GitHub credential, the network, and `git`, then retry."
                    .to_owned(),
            },
            Self::Io(kind) => AppError::Io {
                context: "preparing the project workspace",
                kind,
            },
        }
    }
}
