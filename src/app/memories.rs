//! Scoped memory use cases (specification §3.5, ADR-0022).

use crate::app::AppError;
use crate::app::directory::Directory;
use crate::app::projects::ProjectStore;
use crate::app::storage::AppStateStore;
use crate::domain::ids::{MemoryId, UniqueIdError, unique_short_id};
use crate::domain::memory::{MEMORY_MAX_PER_SCOPE, Memory, MemoryScope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRecord {
    pub memory: Memory,
    pub created_at: String,
}

/// Memory persistence.
pub trait MemoryStore {
    fn insert_memory(&self, memory: &Memory) -> Result<(), crate::app::storage::StorageError>;

    fn memories(
        &self,
        scope: &MemoryScope,
    ) -> Result<Vec<MemoryRecord>, crate::app::storage::StorageError>;

    fn memory(
        &self,
        id: &MemoryId,
    ) -> Result<Option<MemoryRecord>, crate::app::storage::StorageError>;

    fn memory_count(&self, scope: &MemoryScope) -> Result<u64, crate::app::storage::StorageError>;

    fn delete_memory(&self, id: &MemoryId) -> Result<(), crate::app::storage::StorageError>;
}

/// Creates a memory after validating content, owner scope, and the per-scope
/// cap. Project scope is bound to the active laboratory and never switches
/// context.
pub fn create(
    store: &(impl MemoryStore + Directory + ProjectStore),
    state: &impl AppStateStore,
    scope: MemoryScope,
    content: &str,
) -> Result<Memory, AppError> {
    validate_owner(store, state, &scope)?;
    if contains_configured_secret(content) {
        return Err(AppError::Validation {
            problem: "the memory contains a value that looks like the provider credential."
                .to_owned(),
            hint: "Remove the credential from the memory and try again.".to_owned(),
        });
    }
    let memory = Memory::new(MemoryId::generate(), scope, content).map_err(|error| {
        AppError::Validation {
            problem: error.to_string(),
            hint: "Shorten or correct the memory and try again.".to_owned(),
        }
    })?;
    if store.memory_count(&memory.scope)? >= MEMORY_MAX_PER_SCOPE as u64 {
        return Err(AppError::Validation {
            problem: format!("this scope already has {MEMORY_MAX_PER_SCOPE} memories."),
            hint: "Delete an old memory before adding a new one.".to_owned(),
        });
    }
    let generated = unique_short_id::<crate::app::storage::StorageError>(|candidate| {
        match MemoryId::parse(candidate) {
            Some(id) => Ok(store.memory(&id)?.is_some()),
            None => Ok(true),
        }
    })
    .map_err(map_unique)?;
    let id = MemoryId::parse(&generated).ok_or(AppError::Internal)?;
    let memory = Memory::new(id, memory.scope, &memory.content).map_err(|_| AppError::Internal)?;
    store.insert_memory(&memory)?;
    Ok(memory)
}

/// Lists memories for one scope, newest first.
pub fn list(store: &impl MemoryStore, scope: MemoryScope) -> Result<Vec<MemoryRecord>, AppError> {
    Ok(store.memories(&scope)?)
}

/// Lists memories after validating that the owner exists. An archived agent's
/// existing memories remain listable.
pub fn list_checked(
    store: &(impl MemoryStore + Directory + ProjectStore),
    state: &impl AppStateStore,
    scope: MemoryScope,
) -> Result<Vec<MemoryRecord>, AppError> {
    match &scope {
        MemoryScope::Lab(id) => {
            if store.lab(id)?.is_none() {
                return Err(AppError::NotFound {
                    entity: crate::app::EntityKind::Laboratory,
                    id: id.to_string(),
                });
            }
        }
        MemoryScope::Project(id) => {
            let lab = crate::app::context::active_lab(state)?.require()?;
            if store.project_in_lab(id, &lab)?.is_none() {
                return Err(AppError::NotFound {
                    entity: crate::app::EntityKind::Project,
                    id: id.to_string(),
                });
            }
        }
        MemoryScope::Agent(id) => {
            if store.agent(id)?.is_none() {
                return Err(AppError::NotFound {
                    entity: crate::app::EntityKind::Agent,
                    id: id.to_string(),
                });
            }
        }
    }
    list(store, scope)
}

/// Resolves a memory for the deletion confirmation.
pub fn parse_memory_id(raw_id: &str) -> Result<MemoryId, AppError> {
    MemoryId::parse(raw_id).ok_or_else(|| AppError::Usage {
        message: "a memory identifier must be 8 lowercase hexadecimal characters.".to_owned(),
    })
}

pub fn preflight(store: &impl MemoryStore, raw_id: &str) -> Result<MemoryRecord, AppError> {
    let id = parse_memory_id(raw_id)?;
    store.memory(&id)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Memory,
        id: id.to_string(),
    })
}

pub fn delete(store: &impl MemoryStore, record: &MemoryRecord) -> Result<(), AppError> {
    store.delete_memory(&record.memory.id)?;
    Ok(())
}

fn validate_owner(
    store: &(impl Directory + ProjectStore),
    state: &impl AppStateStore,
    scope: &MemoryScope,
) -> Result<(), AppError> {
    match scope {
        MemoryScope::Lab(id) => {
            if store.lab(id)?.is_none() {
                return Err(AppError::NotFound {
                    entity: crate::app::EntityKind::Laboratory,
                    id: id.to_string(),
                });
            }
        }
        MemoryScope::Project(id) => {
            let lab = crate::app::context::active_lab(state)?.require()?;
            if store.project_in_lab(id, &lab)?.is_none() {
                return Err(AppError::NotFound {
                    entity: crate::app::EntityKind::Project,
                    id: id.to_string(),
                });
            }
        }
        MemoryScope::Agent(id) => {
            let record = store.agent(id)?.ok_or_else(|| AppError::NotFound {
                entity: crate::app::EntityKind::Agent,
                id: id.to_string(),
            })?;
            if record.archived {
                return Err(AppError::Validation {
                    problem: format!("agent `{id}` is archived and cannot receive new memories."),
                    hint: "Use an active agent or list the existing memories.".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn contains_configured_secret(value: &str) -> bool {
    std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .filter(|secret| !secret.is_empty())
        .is_some_and(|secret| value.contains(&secret))
}

fn map_unique(error: UniqueIdError<crate::app::storage::StorageError>) -> AppError {
    match error {
        UniqueIdError::Lookup(storage) => AppError::from(storage),
        UniqueIdError::Exhausted => AppError::Internal,
    }
}

/// Parses a scope selector's identifier.
pub fn parse_lab(raw: &str) -> Result<MemoryScope, AppError> {
    crate::app::labs::parse_lab_id(raw).map(MemoryScope::Lab)
}

pub fn parse_project(raw: &str) -> Result<MemoryScope, AppError> {
    crate::app::projects::parse_project_id(raw).map(MemoryScope::Project)
}

pub fn parse_agent(raw: &str) -> Result<MemoryScope, AppError> {
    crate::app::agents::parse_agent_id(raw).map(MemoryScope::Agent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::directory::LabOverview;
    use crate::app::projects::ProjectOverview;
    use crate::app::storage::StorageError;
    use crate::domain::agent::AgentProfile;
    use crate::domain::ids::{AgentId, LabId, MemoryId, ProjectId};
    use crate::domain::lab::Lab;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Store {
        memories: RefCell<Vec<MemoryRecord>>,
        labs: Vec<LabId>,
        projects: Vec<(ProjectId, LabId)>,
        archived: bool,
    }

    impl MemoryStore for Store {
        fn insert_memory(&self, memory: &Memory) -> Result<(), StorageError> {
            self.memories.borrow_mut().push(MemoryRecord {
                memory: memory.clone(),
                created_at: "2026-01-01T00:00:00Z".to_owned(),
            });
            Ok(())
        }
        fn memories(&self, scope: &MemoryScope) -> Result<Vec<MemoryRecord>, StorageError> {
            Ok(self
                .memories
                .borrow()
                .iter()
                .filter(|record| &record.memory.scope == scope)
                .cloned()
                .collect())
        }
        fn memory(&self, id: &MemoryId) -> Result<Option<MemoryRecord>, StorageError> {
            Ok(self
                .memories
                .borrow()
                .iter()
                .find(|record| &record.memory.id == id)
                .cloned())
        }
        fn memory_count(&self, scope: &MemoryScope) -> Result<u64, StorageError> {
            Ok(self
                .memories
                .borrow()
                .iter()
                .filter(|record| &record.memory.scope == scope)
                .count() as u64)
        }
        fn delete_memory(&self, id: &MemoryId) -> Result<(), StorageError> {
            self.memories
                .borrow_mut()
                .retain(|record| &record.memory.id != id);
            Ok(())
        }
    }

    impl ProjectStore for Store {
        fn insert_project(&self, _: &crate::domain::project::Project) -> Result<(), StorageError> {
            unreachable!()
        }
        fn project(
            &self,
            id: &ProjectId,
        ) -> Result<Option<(ProjectId, LabId, String, Option<String>)>, StorageError> {
            Ok(self
                .projects
                .iter()
                .find(|(p, _)| p == id)
                .map(|(p, lab)| (p.clone(), lab.clone(), "P".to_owned(), None)))
        }
        fn projects(&self, _: &LabId) -> Result<Vec<ProjectOverview>, StorageError> {
            Ok(Vec::new())
        }
        fn project_in_lab(
            &self,
            id: &ProjectId,
            lab: &LabId,
        ) -> Result<Option<ProjectOverview>, StorageError> {
            Ok(self
                .projects
                .iter()
                .find(|(p, l)| p == id && l == lab)
                .map(|(p, _)| ProjectOverview {
                    id: p.clone(),
                    name: "P".to_owned(),
                    github_source: None,
                    request_count: 0,
                    story_count: 0,
                    todo: 0,
                    in_progress: 0,
                    in_review: 0,
                    blocked: 0,
                    done: 0,
                }))
        }
    }

    impl Directory for Store {
        fn insert_lab(&self, _: &Lab) -> Result<(), StorageError> {
            unreachable!()
        }
        fn lab_exists(&self, id: &LabId) -> Result<bool, StorageError> {
            Ok(self.labs.contains(id))
        }
        fn lab(&self, id: &LabId) -> Result<Option<LabOverview>, StorageError> {
            Ok(self.labs.contains(id).then(|| LabOverview {
                id: id.clone(),
                name: "L".to_owned(),
                agent_count: 0,
                project_count: 0,
            }))
        }
        fn labs(&self) -> Result<Vec<LabOverview>, StorageError> {
            Ok(Vec::new())
        }
        fn lab_agents(
            &self,
            _: &LabId,
        ) -> Result<Vec<crate::app::directory::AgentRecord>, StorageError> {
            Ok(Vec::new())
        }
        fn insert_agent(&self, _: &AgentProfile, _: &[LabId]) -> Result<(), StorageError> {
            unreachable!()
        }
        fn agent_exists(&self, _: &AgentId) -> Result<bool, StorageError> {
            Ok(true)
        }
        fn agent(
            &self,
            id: &AgentId,
        ) -> Result<Option<crate::app::directory::AgentRecord>, StorageError> {
            Ok(Some(crate::app::directory::AgentRecord {
                id: id.clone(),
                name: "A".to_owned(),
                role: "lead".to_owned(),
                description: "D".to_owned(),
                skills_json: "[]".to_owned(),
                provider: "deepseek".to_owned(),
                model: "m".to_owned(),
                cv: None,
                archived: self.archived,
            }))
        }
        fn agent_memberships(&self, _: &AgentId) -> Result<Vec<LabOverview>, StorageError> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct State(RefCell<Option<String>>);
    impl AppStateStore for State {
        fn get(&self, _: crate::app::storage::AppStateKey) -> Result<Option<String>, StorageError> {
            Ok(self.0.borrow().clone())
        }
        fn set(
            &self,
            _: crate::app::storage::AppStateKey,
            value: &str,
        ) -> Result<(), StorageError> {
            *self.0.borrow_mut() = Some(value.to_owned());
            Ok(())
        }
        fn clear(&self, _: crate::app::storage::AppStateKey) -> Result<(), StorageError> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }

    fn state() -> State {
        let state = State::default();
        *state.0.borrow_mut() = Some("a3f91c07".to_owned());
        state
    }

    #[test]
    fn memories_are_created_listed_and_deleted_by_scope() {
        let mut store = Store::default();
        let scope = MemoryScope::Lab(LabId::parse("a3f91c07").unwrap());
        store.labs.push(LabId::parse("a3f91c07").unwrap());
        let memory = create(&store, &state(), scope.clone(), "Keep changes small.").unwrap();
        assert_eq!(list(&store, scope.clone()).unwrap().len(), 1);
        let record = preflight(&store, memory.id.as_str()).unwrap();
        delete(&store, &record).unwrap();
        assert!(list(&store, scope).unwrap().is_empty());
    }

    #[test]
    fn a_cross_laboratory_project_scope_is_rejected() {
        let mut store = Store::default();
        let project = ProjectId::parse("0b1c2d3e").unwrap();
        store
            .projects
            .push((project.clone(), LabId::parse("1c2d3e4f").unwrap()));
        let error = create(&store, &state(), MemoryScope::Project(project), "x").unwrap_err();
        assert!(matches!(error, AppError::NotFound { .. }), "{error:?}");
    }

    #[test]
    fn an_archived_agent_cannot_receive_a_new_memory() {
        let store = Store {
            archived: true,
            ..Store::default()
        };
        let error = create(
            &store,
            &state(),
            MemoryScope::Agent(AgentId::parse("1c2d3e4f").unwrap()),
            "x",
        )
        .unwrap_err();
        assert!(matches!(error, AppError::Validation { .. }), "{error:?}");
    }
}
