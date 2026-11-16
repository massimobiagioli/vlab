//! Project checkout workspace (ADR-0032).
//!
//! The application decides *when* a project is cloned; the infrastructure
//! adapter decides *how*. No git output or credential crosses this boundary.

use std::io;
use std::path::{Path, PathBuf};

use crate::app::AppError;
use crate::domain::ids::ProjectId;
use crate::domain::project::{FolderSource, GithubSource};

/// A local checkout for one project.
pub trait ProjectWorkspace {
    /// Clones `source` into a new directory for `id`.
    fn clone_project(
        &self,
        source: &GithubSource,
        id: &ProjectId,
    ) -> Result<PathBuf, WorkspaceError>;

    /// Copies a local folder into a new isolated directory for `id`, initializes
    /// a local git repository, and returns the canonical source.
    ///
    /// The default fails; the SQLite-free test doubles that only cover GitHub
    /// keep the smaller implementation.
    fn import_folder(
        &self,
        _source: &FolderSource,
        _id: &ProjectId,
    ) -> Result<ImportedFolder, WorkspaceError> {
        Err(WorkspaceError::ImportFailed)
    }

    /// Removes a checkout after a failed project insert. Best effort.
    fn remove_project(&self, path: &Path);
}

/// A prepared folder import: the canonical source and its isolated checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedFolder {
    pub source: FolderSource,
    pub path: PathBuf,
}

/// Failures that the workspace adapter can report without exposing file or git
/// output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceError {
    DestinationExists,
    GitUnavailable,
    Timeout,
    Failed,
    SourceNotFound,
    SourceNotDirectory,
    SourceNotUtf8,
    SourceOverlapsWorkspace,
    UnsupportedEntry,
    SourceTooLarge,
    ImportFailed,
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
            Self::SourceNotFound => AppError::Precondition {
                problem: "the folder source does not exist.".to_owned(),
                hint: "Choose an existing absolute folder path and retry.".to_owned(),
            },
            Self::SourceNotDirectory => AppError::Precondition {
                problem: "the folder source is not a directory.".to_owned(),
                hint: "Choose a folder, not a file.".to_owned(),
            },
            Self::SourceNotUtf8 => AppError::Precondition {
                problem: "the folder source path is not valid UTF-8.".to_owned(),
                hint: "Choose a folder whose path can be represented as text.".to_owned(),
            },
            Self::SourceOverlapsWorkspace => AppError::Precondition {
                problem: "the folder source overlaps the VLab workspace.".to_owned(),
                hint: "Choose a folder outside `.workspace/` and outside its parent directories."
                    .to_owned(),
            },
            Self::UnsupportedEntry => AppError::Precondition {
                problem: "the folder source contains a symbolic link or special file.".to_owned(),
                hint: "Remove the link or special file, or choose another folder.".to_owned(),
            },
            Self::SourceTooLarge => AppError::Precondition {
                problem: "the folder source is too large to import.".to_owned(),
                hint: "The limit is 100,000 entries, 1 GiB, or 64 directory levels. Choose a smaller folder."
                    .to_owned(),
            },
            Self::ImportFailed => AppError::Workspace {
                problem: "the folder source could not be imported.".to_owned(),
                hint: "Check permissions, free space, and `git`, then retry; the project was not created."
                    .to_owned(),
            },
            Self::Io(kind) => AppError::Io {
                context: "preparing the project workspace",
                kind,
            },
        }
    }
}
