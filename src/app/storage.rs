//! Storage ports and their typed errors (ADR-0002, ADR-0016).
//!
//! The application declares what it needs from durable storage; `infra::db`
//! implements it with SQLite. Errors carry no SQL, file contents, or stored
//! values, so they can be rendered and logged safely.

use std::io;
use std::path::PathBuf;

use crate::app::AppError;

/// The typed keys allowed in `app_state`. Commands never build raw keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStateKey {
    /// The selected laboratory (specification §3.4).
    ActiveLabId,
}

impl AppStateKey {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActiveLabId => "active_lab_id",
        }
    }
}

/// Reads and writes application state. Keys are typed; values are text.
pub trait AppStateStore {
    fn get(&self, key: AppStateKey) -> Result<Option<String>, StorageError>;

    fn set(&self, key: AppStateKey, value: &str) -> Result<(), StorageError>;

    fn clear(&self, key: AppStateKey) -> Result<(), StorageError>;
}

/// A local database failure, already classified for the user.
///
/// Variants hold paths and error kinds only. SQL, stored values, and driver
/// messages are never retained because they may embed user data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    #[error("the database at `{}` is in use by another process.", path.display())]
    Locked { path: PathBuf },

    #[error("the database at `{}` cannot be accessed: permission denied.", path.display())]
    PermissionDenied { path: PathBuf },

    #[error("the database at `{}` is corrupt or is not a SQLite database.", path.display())]
    Corrupt { path: PathBuf },

    #[error("the disk is full; the database at `{}` could not be written.", path.display())]
    DiskFull { path: PathBuf },

    #[error("the database at `{}` has schema version {found}, newer than this build supports ({supported}).", path.display())]
    NewerSchema {
        path: PathBuf,
        found: i64,
        supported: i64,
    },

    #[error("the database migration failed; the database was left unchanged.")]
    Migration,

    #[error("the database's migration history does not match this build.")]
    SchemaMismatch,

    #[error("the database at `{}` could not be opened: {kind}.", path.display())]
    Io { path: PathBuf, kind: io::ErrorKind },

    #[error("a database operation failed.")]
    Operation,
}

impl StorageError {
    /// The next useful action for the user.
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Locked { .. } => "Wait for the other vlab process to finish, then retry.",
            Self::PermissionDenied { .. } => "Check the file's ownership and permissions.",
            Self::Corrupt { .. } => {
                "Move the file aside only after confirming it holds no needed data, then retry."
            }
            Self::DiskFull { .. } => "Free disk space, then retry.",
            Self::NewerSchema { .. } => "Use a newer vlab build, or move the database aside.",
            Self::Migration => {
                "Check the database's free space and permissions, then retry; no changes were committed."
            }
            Self::SchemaMismatch => {
                "Use the vlab build that created the database, or move the database aside."
            }
            Self::Io { .. } => {
                "Check that the path exists, is writable, and has free space, then retry."
            }
            Self::Operation => "Retry; if the problem persists, check the database file.",
        }
    }
}

impl From<StorageError> for AppError {
    fn from(error: StorageError) -> Self {
        AppError::Storage {
            problem: error.to_string(),
            hint: error.hint().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    use super::*;

    #[test]
    fn every_storage_failure_maps_to_a_storage_error_with_a_problem_and_a_hint() {
        let path = PathBuf::from("/data/vlab.db");
        let cases = [
            StorageError::Locked { path: path.clone() },
            StorageError::PermissionDenied { path: path.clone() },
            StorageError::Corrupt { path: path.clone() },
            StorageError::DiskFull { path: path.clone() },
            StorageError::NewerSchema {
                path: path.clone(),
                found: 2,
                supported: 1,
            },
            StorageError::Migration,
            StorageError::SchemaMismatch,
            StorageError::Io {
                path: path.clone(),
                kind: ErrorKind::PermissionDenied,
            },
            StorageError::Operation,
        ];
        for error in cases {
            let app = AppError::from(error.clone());
            assert!(matches!(app, AppError::Storage { .. }), "{error:?}");
            assert!(!app.to_string().is_empty(), "{error:?}");
            assert!(!app.hint().is_empty(), "{error:?}");
        }
    }

    #[test]
    fn messages_never_repeat_stored_values() {
        let error = StorageError::NewerSchema {
            path: PathBuf::from("/data/vlab.db"),
            found: 2,
            supported: 1,
        };
        assert!(!error.to_string().contains("sk-vlab-SECRET-MARKER"));
        assert!(!error.hint().contains("sk-vlab-SECRET-MARKER"));
    }

    #[test]
    fn state_keys_are_stable_tokens() {
        assert_eq!(AppStateKey::ActiveLabId.as_str(), "active_lab_id");
    }
}
