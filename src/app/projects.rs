//! Project use cases (specification §8.1–§8.2).

use crate::app::AppError;
use crate::app::directory::Directory;
use crate::app::storage::AppStateStore;
use crate::domain::ids::{LabId, ProjectId, UniqueIdError, unique_short_id};
use crate::domain::name::Name;
use crate::domain::project::Project;

/// One project row with its planning counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectOverview {
    pub id: ProjectId,
    pub name: String,
    pub github_source: Option<String>,
    pub request_count: u64,
    pub story_count: u64,
    pub todo: u64,
    pub in_progress: u64,
    pub in_review: u64,
    pub blocked: u64,
    pub done: u64,
}

/// Project persistence used by the commands.
pub trait ProjectStore {
    fn insert_project(&self, project: &Project) -> Result<(), crate::app::storage::StorageError>;

    #[allow(clippy::type_complexity)]
    fn project(
        &self,
        id: &ProjectId,
    ) -> Result<Option<(ProjectId, LabId, String, Option<String>)>, crate::app::storage::StorageError>;

    fn projects(
        &self,
        lab: &LabId,
    ) -> Result<Vec<ProjectOverview>, crate::app::storage::StorageError>;

    fn project_in_lab(
        &self,
        id: &ProjectId,
        lab: &LabId,
    ) -> Result<Option<ProjectOverview>, crate::app::storage::StorageError>;
}

/// `--new-project`: requires an active laboratory and a valid name.
pub fn create(
    store: &(impl ProjectStore + Directory),
    state: &impl AppStateStore,
    raw_name: &str,
    github_source: Option<crate::domain::project::GithubSource>,
) -> Result<Project, AppError> {
    let lab_id = crate::app::context::active_lab(state)?.require()?;
    let name = crate::app::labs::validated_name(raw_name)?;
    let generated = unique_short_id(|candidate| match ProjectId::parse(candidate) {
        Some(id) => Ok(store.project(&id)?.is_some()),
        None => Ok(true),
    })
    .map_err(map_unique)?;
    let id = ProjectId::parse(&generated).ok_or(AppError::Internal)?;
    let project = Project::new(id, lab_id, name, github_source);
    store.insert_project(&project)?;
    Ok(project)
}

/// `--list-projects`.
pub fn list(
    store: &impl ProjectStore,
    state: &impl AppStateStore,
) -> Result<Vec<ProjectOverview>, AppError> {
    let lab_id = crate::app::context::active_lab(state)?.require()?;
    Ok(store.projects(&lab_id)?)
}

/// Parses a project identifier; malformed input is a usage error (exit 2).
pub fn parse_project_id(raw: &str) -> Result<ProjectId, AppError> {
    ProjectId::parse(raw).ok_or_else(|| AppError::Usage {
        message: "a project identifier must be 8 lowercase hexadecimal characters.".to_owned(),
    })
}

fn map_unique(error: UniqueIdError<crate::app::storage::StorageError>) -> AppError {
    match error {
        UniqueIdError::Lookup(storage) => AppError::from(storage),
        UniqueIdError::Exhausted => AppError::Internal,
    }
}

/// Validates a project name before storage is opened.
pub fn validated_name(raw: &str) -> Result<Name, AppError> {
    crate::app::labs::validated_name(raw)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;
    use crate::app::directory::{AgentRecord, Directory, LabOverview};
    use crate::app::storage::{AppStateKey, StorageError};
    use crate::domain::agent::AgentProfile;
    use crate::domain::ids::{AgentId, LabId};
    use crate::domain::lab::Lab;

    #[derive(Default)]
    struct Store {
        projects: RefCell<HashMap<ProjectId, (LabId, String)>>,
        inserts: RefCell<u64>,
    }

    impl ProjectStore for Store {
        fn insert_project(&self, project: &Project) -> Result<(), StorageError> {
            *self.inserts.borrow_mut() += 1;
            self.projects.borrow_mut().insert(
                project.id.clone(),
                (project.lab_id.clone(), project.name.to_string()),
            );
            Ok(())
        }
        fn project(
            &self,
            id: &ProjectId,
        ) -> Result<Option<(ProjectId, LabId, String, Option<String>)>, StorageError> {
            Ok(self
                .projects
                .borrow()
                .get(id)
                .map(|(lab, name)| (id.clone(), lab.clone(), name.clone(), None)))
        }
        fn projects(&self, lab: &LabId) -> Result<Vec<ProjectOverview>, StorageError> {
            Ok(self
                .projects
                .borrow()
                .iter()
                .filter(|(_, (owner, _))| owner == lab)
                .map(|(id, (_, name))| ProjectOverview {
                    id: id.clone(),
                    name: name.clone(),
                    github_source: None,
                    request_count: 0,
                    story_count: 0,
                    todo: 0,
                    in_progress: 0,
                    in_review: 0,
                    blocked: 0,
                    done: 0,
                })
                .collect())
        }
        fn project_in_lab(
            &self,
            id: &ProjectId,
            lab: &LabId,
        ) -> Result<Option<ProjectOverview>, StorageError> {
            Ok(self.projects.borrow().get(id).and_then(|(owner, name)| {
                (owner == lab).then(|| ProjectOverview {
                    id: id.clone(),
                    name: name.clone(),
                    github_source: None,
                    request_count: 0,
                    story_count: 0,
                    todo: 0,
                    in_progress: 0,
                    in_review: 0,
                    blocked: 0,
                    done: 0,
                })
            }))
        }
    }

    impl Directory for Store {
        fn insert_lab(&self, _: &Lab) -> Result<(), StorageError> {
            unreachable!()
        }
        fn lab_exists(&self, _: &LabId) -> Result<bool, StorageError> {
            Ok(true)
        }
        fn lab(&self, _: &LabId) -> Result<Option<LabOverview>, StorageError> {
            Ok(None)
        }
        fn labs(&self) -> Result<Vec<LabOverview>, StorageError> {
            Ok(Vec::new())
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
    struct State(RefCell<Option<String>>);
    impl AppStateStore for State {
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

    fn active() -> State {
        let state = State::default();
        *state.0.borrow_mut() = Some("a3f91c07".to_owned());
        state
    }

    #[test]
    fn creating_a_project_requires_an_active_laboratory_and_valid_name() {
        let store = Store::default();
        let error = create(&store, &State::default(), "Portal", None).unwrap_err();
        assert!(matches!(error, AppError::NoActiveLab), "{error:?}");
        assert_eq!(*store.inserts.borrow(), 0);

        let error = create(&store, &active(), "   ", None).unwrap_err();
        assert!(matches!(error, AppError::Usage { .. }), "{error:?}");
        assert_eq!(*store.inserts.borrow(), 0);
    }

    #[test]
    fn creating_and_listing_projects_is_scoped_to_the_active_laboratory() {
        let store = Store::default();
        let state = active();
        let project = create(&store, &state, "Customer Portal", None).unwrap();
        assert_eq!(project.lab_id.as_str(), "a3f91c07");
        let listed = list(&store, &state).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Customer Portal");
    }

    #[test]
    fn malformed_project_ids_are_usage_errors() {
        for raw in ["", "BAD", "0b1c2d3"] {
            assert!(matches!(parse_project_id(raw), Err(AppError::Usage { .. })));
        }
    }
}
