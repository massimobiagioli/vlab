//! Active-laboratory context (specification §3.4, A-04).
//!
//! The selection is stored in SQLite `app_state` through the typed
//! [`AppStateStore`] port. This module interprets the stored value, owns the
//! missing-context error, and is the only place that knows the key.

use crate::app::AppError;
use crate::app::storage::{AppStateKey, AppStateStore};
use crate::domain::ids::LabId;

/// The active laboratory as read from storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveLab {
    Selected(LabId),
    NotSelected,
    /// A stored value that is not a valid identifier. Treated as no
    /// selection, with a warning.
    Invalid,
}

impl ActiveLab {
    pub fn from_stored(value: Option<&str>) -> Self {
        match value {
            None => Self::NotSelected,
            Some(value) => LabId::parse(value).map_or(Self::Invalid, Self::Selected),
        }
    }

    /// The warning to show before acting on this state, if any.
    pub fn warning(&self) -> Option<&'static str> {
        matches!(self, Self::Invalid)
            .then_some("the stored active laboratory is invalid and was ignored.")
    }

    /// The selected laboratory, or the §3.4 missing-context error.
    pub fn require(self) -> Result<LabId, AppError> {
        match self {
            Self::Selected(id) => Ok(id),
            Self::NotSelected | Self::Invalid => Err(no_active_lab()),
        }
    }
}

/// The error for commands that need an active laboratory and have none.
pub fn no_active_lab() -> AppError {
    AppError::NoActiveLab
}

/// Reads the selection. A corrupt stored value becomes [`ActiveLab::Invalid`],
/// which behaves as no selection and carries a warning.
pub fn active_lab(store: &impl AppStateStore) -> Result<ActiveLab, AppError> {
    let stored = store.get(AppStateKey::ActiveLabId)?;
    Ok(ActiveLab::from_stored(stored.as_deref()))
}

/// Stores `id` as the active laboratory. The write is a single statement.
pub fn save_active_lab(store: &impl AppStateStore, id: &LabId) -> Result<(), AppError> {
    store
        .set(AppStateKey::ActiveLabId, id.as_str())
        .map_err(AppError::from)
}

/// Removes the selection and restores the no-context state.
pub fn clear_active_lab(store: &impl AppStateStore) -> Result<(), AppError> {
    store
        .clear(AppStateKey::ActiveLabId)
        .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::app::storage::StorageError;

    #[derive(Default)]
    struct MemoryStore(RefCell<Option<String>>);

    impl AppStateStore for MemoryStore {
        fn get(&self, _: AppStateKey) -> Result<Option<String>, StorageError> {
            Ok(self.0.borrow().clone())
        }

        fn set(&self, _: AppStateKey, value: &str) -> Result<(), StorageError> {
            *self.0.borrow_mut() = Some(value.to_owned());
            Ok(())
        }

        fn clear(&self, _: AppStateKey) -> Result<(), StorageError> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }

    #[test]
    fn the_typed_helpers_round_trip_the_selection() {
        let store = MemoryStore::default();
        assert_eq!(active_lab(&store).unwrap(), ActiveLab::NotSelected);

        let id = LabId::parse("a3f91c07").unwrap();
        save_active_lab(&store, &id).unwrap();
        assert_eq!(active_lab(&store).unwrap(), ActiveLab::Selected(id));

        clear_active_lab(&store).unwrap();
        assert_eq!(active_lab(&store).unwrap(), ActiveLab::NotSelected);
    }

    #[test]
    fn a_corrupt_stored_value_behaves_as_no_selection_with_a_warning() {
        let store = MemoryStore(RefCell::new(Some("not-an-id".to_owned())));
        let state = active_lab(&store).unwrap();
        assert_eq!(state, ActiveLab::Invalid);
        assert!(state.warning().is_some());
        assert!(matches!(state.require(), Err(AppError::NoActiveLab)));
    }

    #[test]
    fn the_helper_returns_the_missing_context_error() {
        assert!(matches!(no_active_lab(), AppError::NoActiveLab));
    }

    #[test]
    fn stored_values_are_interpreted_without_panicking() {
        let id = LabId::parse("a3f91c07").unwrap();
        assert_eq!(
            ActiveLab::from_stored(Some("a3f91c07")),
            ActiveLab::Selected(id.clone())
        );
        assert_eq!(ActiveLab::from_stored(None), ActiveLab::NotSelected);
        for corrupt in ["", "A3F91C07", "a3f91c07\n", "x".repeat(10_000).as_str()] {
            assert_eq!(ActiveLab::from_stored(Some(corrupt)), ActiveLab::Invalid);
        }
        assert_eq!(ActiveLab::Selected(id.clone()).require().unwrap(), id);
    }

    #[test]
    fn invalid_and_missing_selections_require_a_laboratory() {
        assert!(ActiveLab::Invalid.warning().is_some());
        assert!(ActiveLab::NotSelected.warning().is_none());
        for state in [ActiveLab::NotSelected, ActiveLab::Invalid] {
            assert!(matches!(state.require(), Err(AppError::NoActiveLab)));
        }
    }
}
