//! Agent listing, detail, and creation use cases (specification §7.1–§7.4).

pub mod wizard;

use crate::app::AppError;
use crate::app::directory::{AgentRecord, Directory, LabOverview};
use crate::app::storage::AppStateStore;
use crate::domain::agent::AgentProfile;
use crate::domain::ids::AgentId;

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
