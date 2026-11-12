//! `--edit-settings` (specification §5.3): edit a private copy, validate it,
//! and replace the settings file only if the copy is valid.

use crate::app::AppError;
use crate::app::settings::{Editor, LoadError, SettingsDraft, SettingsReport, SettingsStore};

pub fn edit(
    store: &impl SettingsStore,
    editor: &impl Editor,
    credential_present: bool,
    github_credential_present: bool,
) -> Result<SettingsReport, AppError> {
    // Every early return drops the draft, which deletes it.
    let draft = store.create_draft()?;
    editor.edit(draft.path())?;
    let settings = draft.load().map_err(invalid_edit)?;
    draft.commit()?;
    Ok(SettingsReport {
        path: store.path().to_path_buf(),
        settings,
        credential_present,
        github_credential_present,
    })
}

fn invalid_edit(error: LoadError) -> AppError {
    let kept = "the previous settings were kept";
    match error {
        LoadError::Invalid(reason) => AppError::Configuration {
            problem: format!("the edited settings are invalid: {reason}; {kept}."),
            hint: "Run `vlab --edit-settings` again and correct the file.".to_owned(),
        },
        LoadError::Missing => AppError::Configuration {
            problem: format!("the editor deleted the file being edited; {kept}."),
            hint: "Run `vlab --edit-settings` again and save the file in place.".to_owned(),
        },
        LoadError::Unreadable(kind) => AppError::Configuration {
            problem: format!("the edited settings cannot be read: {kind}; {kept}."),
            hint: "Run `vlab --edit-settings` again.".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use super::*;
    use crate::domain::settings::Settings;

    #[derive(Default)]
    struct Log(Rc<RefCell<Vec<&'static str>>>);

    struct FakeStore {
        path: PathBuf,
        draft: Result<Settings, LoadError>,
        log: Rc<RefCell<Vec<&'static str>>>,
    }

    struct FakeDraft {
        path: PathBuf,
        content: Result<Settings, LoadError>,
        log: Rc<RefCell<Vec<&'static str>>>,
        committed: bool,
    }

    impl SettingsStore for FakeStore {
        type Draft = FakeDraft;
        fn path(&self) -> &Path {
            &self.path
        }
        fn load(&self) -> Result<Settings, LoadError> {
            unreachable!("edit never loads the current file directly")
        }
        fn create_draft(&self) -> Result<FakeDraft, AppError> {
            self.log.borrow_mut().push("created");
            Ok(FakeDraft {
                path: PathBuf::from("/c/.settings.yaml.0.tmp"),
                content: self.draft.clone(),
                log: Rc::clone(&self.log),
                committed: false,
            })
        }
    }

    impl SettingsDraft for FakeDraft {
        fn path(&self) -> &Path {
            &self.path
        }
        fn load(&self) -> Result<Settings, LoadError> {
            self.content.clone()
        }
        fn commit(mut self) -> Result<(), AppError> {
            self.committed = true;
            self.log.borrow_mut().push("committed");
            Ok(())
        }
    }

    impl Drop for FakeDraft {
        fn drop(&mut self) {
            if !self.committed {
                self.log.borrow_mut().push("discarded");
            }
        }
    }

    struct FakeEditor(Result<(), AppError>);

    impl Editor for FakeEditor {
        fn edit(&self, file: &Path) -> Result<(), AppError> {
            assert_eq!(file, Path::new("/c/.settings.yaml.0.tmp"));
            match &self.0 {
                Ok(()) => Ok(()),
                Err(_) => Err(AppError::Cancelled),
            }
        }
    }

    fn store(draft: Result<Settings, LoadError>) -> (FakeStore, Log) {
        let log = Log::default();
        let store = FakeStore {
            path: PathBuf::from("/c/settings.yaml"),
            draft,
            log: Rc::clone(&log.0),
        };
        (store, log)
    }

    fn valid() -> Settings {
        Settings::parse("deepseek", "deepseek-chat").unwrap()
    }

    #[test]
    fn a_valid_edit_is_committed() {
        let (store, log) = store(Ok(valid()));
        let report = edit(&store, &FakeEditor(Ok(())), true, false).unwrap();
        assert_eq!(report.settings, valid());
        assert!(report.credential_present);
        assert!(!report.github_credential_present);
        assert_eq!(*log.0.borrow(), ["created", "committed"]);
    }

    #[test]
    fn an_invalid_edit_is_discarded_and_explained() {
        let (store, log) = store(Err(LoadError::Invalid("bad".into())));
        let error = edit(&store, &FakeEditor(Ok(())), false, false).unwrap_err();
        assert_eq!(
            error.to_string(),
            "the edited settings are invalid: bad; the previous settings were kept."
        );
        assert_eq!(*log.0.borrow(), ["created", "discarded"]);
    }

    #[test]
    fn an_unsuccessful_editor_cancels_and_discards() {
        let (store, log) = store(Ok(valid()));
        let error = edit(&store, &FakeEditor(Err(AppError::Cancelled)), false, false).unwrap_err();
        assert!(matches!(error, AppError::Cancelled));
        assert_eq!(*log.0.borrow(), ["created", "discarded"]);
    }
}
