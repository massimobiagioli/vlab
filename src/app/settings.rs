//! Settings use cases (specification §5) and the ports they consume.

use std::io;
use std::path::{Path, PathBuf};

use crate::app::AppError;
use crate::domain::settings::Settings;

/// Why a settings document could not be loaded.
///
/// `Invalid` carries a reason composed by the adapter from typed parser
/// information; it never contains text from the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    Missing,
    Unreadable(io::ErrorKind),
    /// The file exists but is readable by group or other users on Unix.
    Exposed,
    Invalid(String),
}

/// The settings file.
pub trait SettingsStore {
    type Draft: SettingsDraft;

    fn path(&self) -> &Path;

    fn load(&self) -> Result<Settings, LoadError>;

    /// A private copy of the current file, or of the template when there is
    /// none, for the editor to change.
    fn create_draft(&self) -> Result<Self::Draft, AppError>;
}

/// An editable copy of the settings file. Dropping it without
/// [`SettingsDraft::commit`] deletes it and leaves the settings unchanged.
pub trait SettingsDraft {
    fn path(&self) -> &Path;

    fn load(&self) -> Result<Settings, LoadError>;

    /// Atomically replaces the settings file with this draft.
    fn commit(self) -> Result<(), AppError>;
}

/// Opens a file in the user's editor and waits for it to close.
pub trait Editor {
    /// Fails with [`AppError::Cancelled`] when the editor exits unsuccessfully.
    fn edit(&self, file: &Path) -> Result<(), AppError>;
}

/// Valid settings and where they are stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsReport {
    pub path: PathBuf,
    pub settings: Settings,
}

/// `--show-settings`: reads and validates the file. Makes no provider call.
pub fn show(store: &impl SettingsStore) -> Result<SettingsReport, AppError> {
    let settings = store
        .load()
        .map_err(|error| show_error(store.path(), error))?;
    Ok(SettingsReport {
        path: store.path().to_path_buf(),
        settings,
    })
}

fn show_error(path: &Path, error: LoadError) -> AppError {
    let path = path.display();
    match error {
        LoadError::Missing => AppError::Configuration {
            problem: format!("the settings file `{path}` does not exist."),
            hint: "Run `vlab --edit-settings` to create it.".to_owned(),
        },
        LoadError::Unreadable(kind) => AppError::Configuration {
            problem: format!("the settings file `{path}` cannot be read: {kind}."),
            hint: "Check the file's ownership and permissions.".to_owned(),
        },
        LoadError::Exposed => AppError::Configuration {
            problem: format!("the settings file `{path}` is accessible by other users."),
            hint: format!("Run `chmod 600 {path}`."),
        },
        LoadError::Invalid(reason) => AppError::Configuration {
            problem: format!("the settings file `{path}` is invalid: {reason}."),
            hint: "Run `vlab --edit-settings` to fix it.".to_owned(),
        },
    }
}
