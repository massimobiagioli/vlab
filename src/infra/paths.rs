//! Per-user locations and private storage (ADR-0002, ADR-0010, ADR-0031).
//!
//! Nothing here runs unless a command needs storage, so help, version, and
//! argument errors never touch the filesystem. The editor's temporary copy of
//! `settings.yaml` lives in [`Paths::config_dir`] and is created through
//! [`create_private_file`].

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use crate::app::AppError;

const APP_DIR: &str = "vlab";
const SETTINGS_FILE: &str = "settings.yaml";
const DATABASE_FILE: &str = "vlab.db";

pub const CONFIG_DIR_VAR: &str = "VLAB_CONFIG_DIR";
pub const DATA_DIR_VAR: &str = "VLAB_DATA_DIR";
pub const DB_PATH_VAR: &str = "VLAB_DB_PATH";

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("{variable} must be an absolute path.")]
    InvalidOverride { variable: &'static str },

    #[error("the per-user configuration and data directories cannot be determined.")]
    NoUserDirectory,

    #[error("{} is not a directory.", path.display())]
    NotADirectory { path: PathBuf },

    #[error("{} is accessible by other users.", path.display())]
    NotPrivate { path: PathBuf },

    #[error("{} could not be created: {kind}.", path.display())]
    Io { path: PathBuf, kind: io::ErrorKind },
}

impl From<PathError> for AppError {
    fn from(error: PathError) -> Self {
        let problem = error.to_string();
        match error {
            PathError::InvalidOverride { variable } => AppError::Configuration {
                problem,
                hint: format!(
                    "Set {variable} to an absolute path, or unset it to use the default."
                ),
            },
            PathError::NoUserDirectory => AppError::Configuration {
                problem,
                hint: format!("Set HOME, or set {CONFIG_DIR_VAR} and {DATA_DIR_VAR}."),
            },
            PathError::NotADirectory { .. } => AppError::Precondition {
                problem,
                hint: "Move the file or link away, or point the override elsewhere.".to_owned(),
            },
            PathError::NotPrivate { path } => AppError::Precondition {
                problem,
                hint: format!(
                    "Run `chmod 700 {}` or choose another directory.",
                    path.display()
                ),
            },
            PathError::Io { .. } => AppError::Precondition {
                problem,
                hint: "Check the parent directory's ownership and permissions.".to_owned(),
            },
        }
    }
}

/// Platform base directories, before the `vlab` component is appended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformDirs {
    pub config: PathBuf,
    pub data: PathBuf,
}

impl PlatformDirs {
    /// The current user's directories, or `None` without a home directory.
    pub fn current() -> Option<Self> {
        directories::BaseDirs::new().map(|dirs| Self {
            config: dirs.config_dir().to_path_buf(),
            data: dirs.data_dir().to_path_buf(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    config_dir: PathBuf,
    data_dir: PathBuf,
    database_file: PathBuf,
}

impl Paths {
    /// Resolves paths from the process environment.
    ///
    /// A debug build located at `<repo>/target/debug/vlab` uses
    /// `<repo>/.data` when it exists and no path override is set
    /// (ADR-0031). Explicit overrides always win.
    pub fn from_environment() -> Result<Self, PathError> {
        let overridden = [CONFIG_DIR_VAR, DATA_DIR_VAR, DB_PATH_VAR]
            .iter()
            .any(|variable| std::env::var_os(variable).is_some());
        let repository_data = if cfg!(debug_assertions) && !overridden {
            repository_data_dir()
        } else {
            None
        };
        if let Some(data) = repository_data {
            let data = data.into_os_string();
            return Self::resolve(
                move |key| match key {
                    CONFIG_DIR_VAR | DATA_DIR_VAR => Some(data.clone()),
                    _ => None,
                },
                None,
            );
        }
        Self::resolve(|key| std::env::var_os(key), PlatformDirs::current())
    }

    /// Resolves paths from `lookup` (environment variables) and `platform`.
    ///
    /// Overrides must be absolute so that nothing is ever resolved against
    /// the current working directory.
    pub fn resolve(
        lookup: impl Fn(&str) -> Option<OsString>,
        platform: Option<PlatformDirs>,
    ) -> Result<Self, PathError> {
        let config_override = absolute_override(&lookup, CONFIG_DIR_VAR)?;
        let data_override = absolute_override(&lookup, DATA_DIR_VAR)?;
        let database_override = absolute_override(&lookup, DB_PATH_VAR)?;

        let config_dir = match config_override {
            Some(dir) => dir,
            None => platform
                .as_ref()
                .ok_or(PathError::NoUserDirectory)?
                .config
                .join(APP_DIR),
        };
        let data_dir = match data_override {
            Some(dir) => dir,
            None => platform
                .as_ref()
                .ok_or(PathError::NoUserDirectory)?
                .data
                .join(APP_DIR),
        };
        let database_file = database_override.unwrap_or_else(|| data_dir.join(DATABASE_FILE));

        Ok(Self {
            config_dir,
            data_dir,
            database_file,
        })
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join(SETTINGS_FILE)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn database_file(&self) -> &Path {
        &self.database_file
    }
}

/// The repository `.data` directory for a Cargo debug executable.
///
/// The executable is expected at `<repo>/target/debug/vlab`; the fallback is
/// derived from that path, never from the current working directory.
fn repository_data_dir() -> Option<PathBuf> {
    repository_data_dir_for(&std::env::current_exe().ok()?)
}

fn repository_data_dir_for(exe: &Path) -> Option<PathBuf> {
    let data = exe.parent()?.parent()?.parent()?.join(".data");
    data.is_dir().then_some(data)
}

fn absolute_override(
    lookup: &impl Fn(&str) -> Option<OsString>,
    variable: &'static str,
) -> Result<Option<PathBuf>, PathError> {
    match lookup(variable) {
        None => Ok(None),
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                Ok(Some(path))
            } else {
                Err(PathError::InvalidOverride { variable })
            }
        }
    }
}

/// Creates `dir` and missing parents with mode `0700`, or verifies that an
/// existing `dir` is a real directory closed to group and others.
///
/// Existing permissions are never loosened or changed: an unsafe directory is
/// refused, failing closed.
pub fn ensure_private_dir(dir: &Path) -> Result<(), PathError> {
    let io_error = |error: io::Error| PathError::Io {
        path: dir.to_path_buf(),
        kind: error.kind(),
    };
    match fs::symlink_metadata(dir) {
        Ok(metadata) => verify_private_dir(dir, &metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            private_dir_builder().create(dir).map_err(io_error)?;
            let metadata = fs::symlink_metadata(dir).map_err(io_error)?;
            verify_private_dir(dir, &metadata)
        }
        Err(error) => Err(io_error(error)),
    }
}

/// Creates a new file with mode `0600`. Fails if `path` already exists, so a
/// planted file or link is never reused.
pub fn create_private_file(path: &Path) -> Result<File, PathError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|error| PathError::Io {
        path: path.to_path_buf(),
        kind: error.kind(),
    })
}

fn private_dir_builder() -> fs::DirBuilder {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
}

fn verify_private_dir(dir: &Path, metadata: &fs::Metadata) -> Result<(), PathError> {
    if !metadata.file_type().is_dir() {
        return Err(PathError::NotADirectory {
            path: dir.to_path_buf(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(PathError::NotPrivate {
                path: dir.to_path_buf(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::repository_data_dir_for;
    use std::fs;
    use std::path::Path;

    #[test]
    fn repository_data_is_found_next_to_a_target_debug_binary() {
        let root = tempfile::tempdir().unwrap();
        let debug = root.path().join("target").join("debug");
        fs::create_dir_all(&debug).unwrap();
        let data = root.path().join(".data");
        fs::create_dir(&data).unwrap();
        assert_eq!(repository_data_dir_for(&debug.join("vlab")), Some(data));
    }

    #[test]
    fn repository_data_is_only_used_when_it_exists() {
        let root = tempfile::tempdir().unwrap();
        let debug = root.path().join("target").join("debug");
        fs::create_dir_all(&debug).unwrap();
        assert_eq!(repository_data_dir_for(&debug.join("vlab")), None);
        assert_eq!(repository_data_dir_for(Path::new("/")), None);
    }
}
