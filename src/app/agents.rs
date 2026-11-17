//! Agent listing, detail, and creation use cases (specification §7.1–§7.4).

pub mod wizard;

use crate::app::AppError;
use crate::app::directory::{AgentRecord, Directory, LabOverview};
use crate::app::memories::MemoryStore;
use crate::app::storage::{AppStateStore, StorageError};
use crate::domain::agent::AgentProfile;
use crate::domain::ids::{AgentId, LabId, MemoryId, UniqueIdError, unique_short_id};
use crate::domain::lab::ensure_can_join;
use crate::domain::memory::{MEMORY_MAX_PER_SCOPE, Memory, MemoryScope};
use crate::domain::name::Name;
use crate::domain::preset::PresetAgent;
use crate::domain::settings::Settings;

/// A validated agent together with its memberships.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentListItem {
    pub record: AgentRecord,
    pub projects: u64,
    pub open_stories: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDetail {
    pub profile: AgentProfile,
    pub archived: bool,
    pub memberships: Vec<LabOverview>,
    pub projects: Vec<(crate::domain::ids::ProjectId, String)>,
    pub assigned_stories: u64,
    pub open_stories: u64,
    pub completed_stories: u64,
    pub memories: u64,
}

/// `--list-agents`: active members of the active laboratory, excluding
/// archived profiles. Requires an active laboratory (exit 3 otherwise).
pub fn list(
    store: &impl Directory,
    state: &impl AppStateStore,
) -> Result<Vec<AgentListItem>, AppError> {
    let lab_id = crate::app::context::active_lab(state)?.require()?;
    if store.lab(&lab_id)?.is_none() {
        return Err(AppError::NotFound {
            entity: crate::app::EntityKind::Laboratory,
            id: lab_id.to_string(),
        });
    }
    let mut members = store.lab_agents(&lab_id)?;
    members.retain(|member| !member.archived);
    let mut items = Vec::with_capacity(members.len());
    for record in members {
        items.push(AgentListItem {
            projects: store.agent_project_count(&record.id)?,
            open_stories: store.agent_open_story_count(&record.id)?,
            record,
        });
    }
    Ok(items)
}

/// `--show-agent-detail`: resolves any agent by ID, including an archived one.
pub fn detail(store: &impl Directory, raw_id: &str) -> Result<AgentDetail, AppError> {
    let id = parse_agent_id(raw_id)?;
    let record = store.agent(&id)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Agent,
        id: id.to_string(),
    })?;
    let profile = record.profile().map_err(|field| AppError::Precondition {
        problem: format!("agent `{id}` has invalid stored data: {field}."),
        hint: "Restore the agent from a valid profile or recreate it.".to_owned(),
    })?;
    let memberships = store.agent_memberships(&id)?;
    Ok(AgentDetail {
        profile,
        archived: record.archived,
        memberships,
        projects: store.agent_project_names(&id)?,
        assigned_stories: store.agent_assigned_story_count(&id)?,
        open_stories: store.agent_open_story_count(&id)?,
        completed_stories: store.agent_completed_story_count(&id)?,
        memories: store.agent_memory_count(&id)?,
    })
}

/// Malformed identifiers are usage errors (exit 2); well-formed absent IDs are
/// missing entities (exit 3).
pub fn parse_agent_id(raw: &str) -> Result<AgentId, AppError> {
    AgentId::parse(raw).ok_or_else(|| AppError::Usage {
        message: "an agent identifier must be 8 lowercase hexadecimal characters.".to_owned(),
    })
}

/// Atomic persistence for a duplicated profile, its compatible memberships,
/// and its optional agent-scope memories (ADR-0058).
pub trait AgentCopyStore {
    fn insert_agent_copy(
        &self,
        profile: &AgentProfile,
        memberships: &[LabId],
        memories: &[Memory],
    ) -> Result<(), StorageError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateReport {
    pub profile: AgentProfile,
    pub memberships: Vec<LabId>,
    pub skipped_memberships: usize,
    pub memories: usize,
}

/// Duplicates an agent profile with a new identifier and name.
///
/// Memberships are copied only where the one-Tech-Lead rule allows them. When
/// `preserve_memory` is set, agent-scope memories are copied with new
/// identifiers and the duplicate's creation time, after the credential and
/// memory bounds are checked.
pub fn duplicate_agent(
    store: &(impl Directory + MemoryStore + AgentCopyStore),
    source: &AgentId,
    raw_name: &str,
    preserve_memory: bool,
    secret: Option<&str>,
) -> Result<DuplicateReport, AppError> {
    let record = store.agent(source)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Agent,
        id: source.to_string(),
    })?;
    let source_profile = record.profile().map_err(|field| AppError::Precondition {
        problem: format!("agent `{source}` has invalid stored data: {field}."),
        hint: "Restore the agent from a valid profile or recreate it.".to_owned(),
    })?;
    let name = Name::parse(raw_name).map_err(|error| AppError::Validation {
        problem: error.to_string(),
        hint: "Use a non-empty, single-line name of at most 100 characters.".to_owned(),
    })?;

    let mut memberships = Vec::new();
    let mut skipped_memberships = 0;
    for lab in store.agent_memberships(source)? {
        let existing = store.lab_agents(&lab.id)?;
        let roles = existing
            .iter()
            .filter(|member| !member.archived)
            .filter_map(|member| member.role().ok());
        if ensure_can_join(roles, source_profile.role).is_ok() {
            memberships.push(lab.id);
        } else {
            skipped_memberships += 1;
        }
    }

    let generated = unique_short_id(|candidate| match AgentId::parse(candidate) {
        Some(id) => store.agent_exists(&id),
        None => Ok(true),
    })
    .map_err(map_unique)?;
    let new_id = AgentId::parse(&generated).ok_or(AppError::Internal)?;
    let profile = AgentProfile::new(
        new_id.clone(),
        name,
        source_profile.role,
        source_profile.description.clone(),
        source_profile.skills.clone(),
        source_profile.provider,
        source_profile.model.clone(),
        source_profile.cv.clone(),
    )
    .map_err(|error| AppError::Validation {
        problem: error.to_string(),
        hint: "Correct the source profile and retry.".to_owned(),
    })?;

    let mut memories = Vec::new();
    if preserve_memory {
        for record in store
            .memories(&MemoryScope::Agent(source.clone()))?
            .into_iter()
            .take(MEMORY_MAX_PER_SCOPE)
        {
            if secret
                .is_some_and(|secret| !secret.is_empty() && record.memory.content.contains(secret))
            {
                return Err(AppError::Validation {
                    problem:
                        "a source memory contains a value that looks like the provider credential."
                            .to_owned(),
                    hint: "Remove the credential from the source memory before duplicating."
                        .to_owned(),
                });
            }
            let generated = unique_short_id(|candidate| match MemoryId::parse(candidate) {
                Some(id) => Ok(store.memory(&id)?.is_some()),
                None => Ok(true),
            })
            .map_err(map_unique)?;
            let id = MemoryId::parse(&generated).ok_or(AppError::Internal)?;
            let memory = Memory::new(
                id,
                MemoryScope::Agent(new_id.clone()),
                &record.memory.content,
            )
            .map_err(|error| AppError::Validation {
                problem: error.to_string(),
                hint: "Correct the source memory and retry.".to_owned(),
            })?;
            memories.push(memory);
        }
    }

    store.insert_agent_copy(&profile, &memberships, &memories)?;
    Ok(DuplicateReport {
        profile,
        memberships,
        skipped_memberships,
        memories: memories.len(),
    })
}

/// The result of creating an agent from a markdown definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedDefinitionAgent {
    pub profile: AgentProfile,
    pub memberships: Vec<LabId>,
}

/// Creates an agent from a validated markdown definition.
///
/// The active laboratory is joined when it still exists; a second lead is
/// rejected. Without a usable active laboratory the profile is catalog-only.
pub fn create_from_definition(
    store: &impl Directory,
    state: &impl AppStateStore,
    definition: &PresetAgent,
    settings: &Settings,
) -> Result<CreatedDefinitionAgent, AppError> {
    let target = match crate::app::context::active_lab(state)? {
        crate::app::context::ActiveLab::Selected(id) if store.lab(&id)?.is_some() => Some(id),
        _ => None,
    };
    let mut memberships = Vec::new();
    if let Some(lab) = target {
        let existing = store.lab_agents(&lab)?;
        let roles = existing
            .iter()
            .filter(|member| !member.archived)
            .filter_map(|member| member.role().ok());
        ensure_can_join(roles, definition.role).map_err(|error| AppError::Validation {
            problem: format!("{error} Laboratory: {lab}."),
            hint: "Choose a different role or edit the existing Tech Lead.".to_owned(),
        })?;
        memberships.push(lab);
    }
    let generated = unique_short_id(|candidate| match AgentId::parse(candidate) {
        Some(id) => store.agent_exists(&id),
        None => Ok(true),
    })
    .map_err(map_unique)?;
    let id = AgentId::parse(&generated).ok_or(AppError::Internal)?;
    let profile = AgentProfile::new(
        id,
        definition.name.clone(),
        definition.role,
        definition.description.clone(),
        definition.skills.clone(),
        settings.provider,
        settings.model.clone(),
        definition.cv.clone(),
    )
    .map_err(|error| AppError::Validation {
        problem: error.to_string(),
        hint: "Fix the markdown definition and retry.".to_owned(),
    })?;
    store.insert_agent(&profile, &memberships)?;
    Ok(CreatedDefinitionAgent {
        profile,
        memberships,
    })
}

fn map_unique(error: UniqueIdError<StorageError>) -> AppError {
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
    use crate::app::directory::LabOverview;
    use crate::app::storage::{AppStateKey, StorageError};
    use crate::domain::ids::LabId;

    #[derive(Default)]
    struct SpyDirectory {
        labs: RefCell<HashMap<LabId, LabOverview>>,
        agents: RefCell<HashMap<AgentId, AgentRecord>>,
        memberships: RefCell<HashMap<AgentId, Vec<LabId>>>,
    }

    impl Directory for SpyDirectory {
        fn insert_lab(&self, _: &crate::domain::lab::Lab) -> Result<(), StorageError> {
            unreachable!()
        }
        fn lab_exists(&self, id: &LabId) -> Result<bool, StorageError> {
            Ok(self.labs.borrow().contains_key(id))
        }
        fn lab(&self, id: &LabId) -> Result<Option<LabOverview>, StorageError> {
            Ok(self.labs.borrow().get(id).cloned())
        }
        fn labs(&self) -> Result<Vec<LabOverview>, StorageError> {
            Ok(self.labs.borrow().values().cloned().collect())
        }
        fn lab_agents(&self, lab: &LabId) -> Result<Vec<AgentRecord>, StorageError> {
            Ok(self
                .memberships
                .borrow()
                .iter()
                .filter(|(_, labs)| labs.contains(lab))
                .filter_map(|(id, _)| self.agents.borrow().get(id).cloned())
                .collect())
        }
        fn insert_agent(&self, _: &AgentProfile, _: &[LabId]) -> Result<(), StorageError> {
            unreachable!()
        }
        fn agent_exists(&self, id: &AgentId) -> Result<bool, StorageError> {
            Ok(self.agents.borrow().contains_key(id))
        }
        fn agent(&self, id: &AgentId) -> Result<Option<AgentRecord>, StorageError> {
            Ok(self.agents.borrow().get(id).cloned())
        }
        fn agent_memberships(&self, id: &AgentId) -> Result<Vec<LabOverview>, StorageError> {
            Ok(self
                .memberships
                .borrow()
                .get(id)
                .map(|labs| {
                    labs.iter()
                        .filter_map(|lab| self.labs.borrow().get(lab).cloned())
                        .collect()
                })
                .unwrap_or_default())
        }
    }

    #[derive(Default)]
    struct MemoryState(RefCell<Option<String>>);

    impl AppStateStore for MemoryState {
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

    fn lab(id: &str) -> LabOverview {
        LabOverview {
            id: LabId::parse(id).unwrap(),
            name: "Lab".to_owned(),
            description: String::new(),
            agent_count: 1,
            project_count: 0,
        }
    }

    fn agent(id: &str, archived: bool) -> AgentRecord {
        AgentRecord {
            id: AgentId::parse(id).unwrap(),
            name: "Agent".to_owned(),
            role: "dev".to_owned(),
            description: "Works".to_owned(),
            skills_json: "[\"rust\"]".to_owned(),
            provider: "deepseek".to_owned(),
            model: "deepseek-chat".to_owned(),
            cv: Some("CV".to_owned()),
            archived,
        }
    }

    #[test]
    fn listing_requires_an_active_laboratory() {
        let store = SpyDirectory::default();
        let state = MemoryState::default();
        let error = list(&store, &state).unwrap_err();
        assert!(matches!(error, AppError::NoActiveLab), "{error:?}");
    }

    #[test]
    fn listing_excludes_archived_agents_and_unknown_labs_are_missing() {
        let store = SpyDirectory::default();
        let state = MemoryState::default();
        *state.0.borrow_mut() = Some("a3f91c07".to_owned());
        let error = list(&store, &state).unwrap_err();
        assert!(matches!(error, AppError::NotFound { .. }), "{error:?}");

        let lab = lab("a3f91c07");
        store.labs.borrow_mut().insert(lab.id.clone(), lab.clone());
        let active = agent("0b1c2d3e", false);
        let archived = agent("1c2d3e4f", true);
        store
            .agents
            .borrow_mut()
            .insert(active.id.clone(), active.clone());
        store
            .agents
            .borrow_mut()
            .insert(archived.id.clone(), archived);
        store
            .memberships
            .borrow_mut()
            .insert(active.id.clone(), vec![lab.id.clone()]);
        store
            .memberships
            .borrow_mut()
            .insert(AgentId::parse("1c2d3e4f").unwrap(), vec![lab.id]);
        let listed = list(&store, &state).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].record, active);
        assert_eq!(listed[0].projects, 0);
        assert_eq!(listed[0].open_stories, 0);
    }

    #[test]
    fn detail_shows_profile_memberships_and_archived_status() {
        let store = SpyDirectory::default();
        let lab = lab("a3f91c07");
        let archived = agent("0b1c2d3e", true);
        store.labs.borrow_mut().insert(lab.id.clone(), lab.clone());
        store
            .agents
            .borrow_mut()
            .insert(archived.id.clone(), archived);
        store
            .memberships
            .borrow_mut()
            .insert(AgentId::parse("0b1c2d3e").unwrap(), vec![lab.id.clone()]);
        let detail = detail(&store, "0b1c2d3e").unwrap();
        assert_eq!(detail.profile.id.as_str(), "0b1c2d3e");
        assert_eq!(detail.memberships, vec![lab]);
    }

    #[test]
    fn malformed_ids_are_usage_errors_and_absent_ids_are_missing() {
        let store = SpyDirectory::default();
        for raw in ["", "ABC", "0b1c2d3", "../../x"] {
            assert!(matches!(detail(&store, raw), Err(AppError::Usage { .. })));
        }
        assert!(matches!(
            detail(&store, "0b1c2d3e"),
            Err(AppError::NotFound { .. })
        ));
    }
}
