//! Laboratory use cases (specification §6.1–§6.3).

use crate::app::AppError;
use crate::app::directory::{Directory, LabOverview};
use crate::app::lab_validation::{self, LabCheck, RuleResult};
use crate::app::storage::AppStateStore;
use crate::domain::ids::{LabId, UniqueIdError, unique_short_id};
use crate::domain::lab::Lab;
use crate::domain::name::Name;

/// One row of `--list-labs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabListItem {
    pub lab: LabOverview,
    pub check: LabCheck,
}

/// The result of a successful `--open-lab`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSummary {
    pub lab: LabOverview,
    /// Failed structural rules, shown as warnings. Opening is still allowed.
    pub warnings: Vec<RuleResult>,
}

/// `--new-lab`: validates the name, allocates a collision-free identifier, and
/// inserts exactly one row. The active laboratory is not changed.
pub fn create(store: &impl Directory, raw_name: &str) -> Result<Lab, AppError> {
    let name = validated_name(raw_name)?;
    let generated = unique_short_id(|candidate| match LabId::parse(candidate) {
        Some(id) => store.lab_exists(&id),
        // Generated identifiers are always well formed; treating an
        // impossible malformed value as taken keeps the retry loop safe.
        None => Ok(true),
    })
    .map_err(map_unique)?;
    let id = LabId::parse(&generated).ok_or(AppError::Internal)?;
    let lab = Lab::new(id, name);
    store.insert_lab(&lab)?;
    Ok(lab)
}

/// Validates a laboratory name before any storage is opened, so malformed
/// input creates neither a database nor a row.
pub fn validated_name(raw_name: &str) -> Result<Name, AppError> {
    Name::parse(raw_name).map_err(|error| AppError::Usage {
        message: error.to_string(),
    })
}

/// `--list-labs`: every laboratory with its member count, validation result,
/// and the active marker (applied by the renderer).
pub fn list(store: &impl Directory) -> Result<Vec<LabListItem>, AppError> {
    let mut items = Vec::new();
    for lab in store.labs()? {
        let members = store.lab_agents(&lab.id)?;
        items.push(LabListItem {
            check: lab_validation::check(&lab, &members),
            lab,
        });
    }
    Ok(items)
}

/// `--open-lab`: persists the selection and returns warnings for an
/// incomplete laboratory. Unknown IDs exit 3; malformed IDs exit 2.
pub fn open(
    store: &impl Directory,
    state: &impl AppStateStore,
    raw_id: &str,
) -> Result<OpenSummary, AppError> {
    let id = parse_lab_id(raw_id)?;
    let lab = store.lab(&id)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Laboratory,
        id: id.to_string(),
    })?;
    let members = store.lab_agents(&id)?;
    let check = lab_validation::check(&lab, &members);
    crate::app::context::save_active_lab(state, &id)?;
    Ok(OpenSummary {
        lab,
        warnings: check.failures().cloned().collect(),
    })
}

/// `--check-lab`: loads the laboratory and evaluates every rule.
pub fn check(store: &impl Directory, raw_id: &str) -> Result<LabCheck, AppError> {
    let id = parse_lab_id(raw_id)?;
    let lab = store.lab(&id)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Laboratory,
        id: id.to_string(),
    })?;
    let members = store.lab_agents(&id)?;
    Ok(lab_validation::check(&lab, &members))
}

/// Parses an identifier supplied on the command line. Malformed input is a
/// usage problem (exit 2), not a missing entity (exit 3).
pub fn parse_lab_id(raw: &str) -> Result<LabId, AppError> {
    LabId::parse(raw).ok_or_else(|| AppError::Usage {
        message: "a laboratory identifier must be 8 lowercase hexadecimal characters.".to_owned(),
    })
}

fn map_unique(error: UniqueIdError<crate::app::storage::StorageError>) -> AppError {
    match error {
        UniqueIdError::Lookup(storage) => AppError::from(storage),
        UniqueIdError::Exhausted => AppError::Internal,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;
    use crate::app::directory::{AgentRecord, Directory};
    use crate::app::storage::StorageError;
    use crate::domain::agent::AgentProfile;
    use crate::domain::ids::AgentId;
    use crate::domain::lab::Lab;

    #[derive(Default)]
    struct SpyDirectory {
        labs: RefCell<HashMap<LabId, LabOverview>>,
        queries: RefCell<Vec<String>>,
        fail_insert: bool,
    }

    impl Directory for SpyDirectory {
        fn insert_lab(&self, lab: &Lab) -> Result<(), StorageError> {
            if self.fail_insert {
                return Err(StorageError::Operation);
            }
            self.labs.borrow_mut().insert(
                lab.id.clone(),
                LabOverview {
                    id: lab.id.clone(),
                    name: lab.name.to_string(),
                    agent_count: 0,
                    project_count: 0,
                },
            );
            Ok(())
        }
        fn lab_exists(&self, id: &LabId) -> Result<bool, StorageError> {
            self.queries.borrow_mut().push(format!("exists {id}"));
            Ok(self.labs.borrow().contains_key(id))
        }
        fn lab(&self, id: &LabId) -> Result<Option<LabOverview>, StorageError> {
            Ok(self.labs.borrow().get(id).cloned())
        }
        fn labs(&self) -> Result<Vec<LabOverview>, StorageError> {
            Ok(self.labs.borrow().values().cloned().collect())
        }
        fn lab_agents(&self, _: &LabId) -> Result<Vec<AgentRecord>, StorageError> {
            Ok(Vec::new())
        }
        fn insert_agent(&self, _: &AgentProfile, _: &[LabId]) -> Result<(), StorageError> {
            unreachable!()
        }
        fn agent_exists(&self, _: &AgentId) -> Result<bool, StorageError> {
            unreachable!()
        }
        fn agent(&self, _: &AgentId) -> Result<Option<AgentRecord>, StorageError> {
            unreachable!()
        }
        fn agent_memberships(&self, _: &AgentId) -> Result<Vec<LabOverview>, StorageError> {
            unreachable!()
        }
    }

    #[derive(Default)]
    struct MemoryState {
        value: RefCell<Option<String>>,
        fail_set: bool,
    }

    impl AppStateStore for MemoryState {
        fn get(&self, _: crate::app::storage::AppStateKey) -> Result<Option<String>, StorageError> {
            Ok(self.value.borrow().clone())
        }
        fn set(
            &self,
            _: crate::app::storage::AppStateKey,
            value: &str,
        ) -> Result<(), StorageError> {
            if self.fail_set {
                return Err(StorageError::Operation);
            }
            *self.value.borrow_mut() = Some(value.to_owned());
            Ok(())
        }
        fn clear(&self, _: crate::app::storage::AppStateKey) -> Result<(), StorageError> {
            *self.value.borrow_mut() = None;
            Ok(())
        }
    }

    #[test]
    fn creating_a_laboratory_persists_one_row_and_leaves_the_selection_alone() {
        let store = SpyDirectory::default();
        let state = MemoryState::default();
        let lab = create(&store, "  Payments Platform  ").unwrap();
        assert_eq!(lab.name.as_str(), "Payments Platform");
        assert_eq!(store.labs.borrow().len(), 1);
        assert_eq!(state.value.borrow().clone(), None);

        let items = list(&store).unwrap();
        assert_eq!(items.len(), 1);
        assert!(!items[0].check.valid(), "a new laboratory is incomplete");
    }

    #[test]
    fn empty_names_are_usage_errors_and_write_nothing() {
        let store = SpyDirectory::default();
        let error = create(&store, "   ").unwrap_err();
        assert!(matches!(error, AppError::Usage { .. }), "{error:?}");
        assert!(store.labs.borrow().is_empty());
    }

    #[test]
    fn malformed_ids_never_reach_the_store() {
        let store = SpyDirectory::default();
        let state = MemoryState::default();
        for raw in ["", "A3F91C07", "a3f91c0", "../../x"] {
            let error = open(&store, &state, raw).unwrap_err();
            assert!(
                matches!(error, AppError::Usage { .. }),
                "{raw:?}: {error:?}"
            );
            assert!(matches!(check(&store, raw), Err(AppError::Usage { .. })));
        }
        assert!(store.queries.borrow().is_empty());
    }

    #[test]
    fn unknown_but_well_formed_ids_are_not_found() {
        let store = SpyDirectory::default();
        let state = MemoryState::default();
        let error = open(&store, &state, "a3f91c07").unwrap_err();
        assert!(matches!(error, AppError::NotFound { .. }), "{error:?}");
        assert!(state.value.borrow().is_none());
    }

    #[test]
    fn opening_stores_the_selection_and_reports_incomplete_laboratories() {
        let store = SpyDirectory::default();
        let state = MemoryState::default();
        let lab = create(&store, "Payments").unwrap();
        let summary = open(&store, &state, lab.id.as_str()).unwrap();
        assert_eq!(summary.lab.id, lab.id);
        assert!(!summary.warnings.is_empty());
        assert_eq!(state.value.borrow().as_deref(), Some(lab.id.as_str()));
    }

    #[test]
    fn a_failed_selection_write_leaves_the_previous_selection_unchanged() {
        let store = SpyDirectory::default();
        let lab = create(&store, "Payments").unwrap();
        let state = MemoryState {
            value: RefCell::new(Some("1c2d3e4f".to_owned())),
            fail_set: true,
        };
        let error = open(&store, &state, lab.id.as_str()).unwrap_err();
        assert!(matches!(error, AppError::Storage { .. }), "{error:?}");
        assert_eq!(state.value.borrow().as_deref(), Some("1c2d3e4f"));
    }

    #[test]
    fn a_failed_insert_does_not_report_success() {
        let store = SpyDirectory {
            fail_insert: true,
            ..SpyDirectory::default()
        };
        let error = create(&store, "Payments").unwrap_err();
        assert!(matches!(error, AppError::Storage { .. }), "{error:?}");
    }
}
