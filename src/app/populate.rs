//! Laboratory membership editing for the populate screen (ADR-0053).

use std::collections::HashSet;

use crate::app::AppError;
use crate::app::directory::{AgentRecord, Directory, LabOverview};
use crate::domain::agent::Role;
use crate::domain::ids::{AgentId, LabId};

/// One agent as shown in a populate pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopulateAgent {
    pub id: AgentId,
    pub name: String,
    pub role: String,
    pub description: String,
    pub skills: Vec<String>,
}

impl PopulateAgent {
    pub fn is_lead(&self) -> bool {
        self.role == Role::Lead.as_str()
    }
}

/// Everything the populate screen reads in one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopulateSnapshot {
    pub lab: LabOverview,
    pub available: Vec<PopulateAgent>,
    pub members: Vec<PopulateAgent>,
    /// Archived members are preserved in storage but not shown.
    pub archived_members: usize,
}

/// What one save changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopulateReport {
    pub added: usize,
    pub removed: usize,
    pub members: usize,
}

/// Loads the selected laboratory, its active members, and every active profile
/// that is not yet a member. Archived profiles never appear in either pane.
pub fn show(store: &impl Directory, lab_id: &LabId) -> Result<PopulateSnapshot, AppError> {
    let lab = store.lab(lab_id)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Laboratory,
        id: lab_id.to_string(),
    })?;
    let records = store.lab_agents(lab_id)?;
    let archived_members = records.iter().filter(|record| record.archived).count();
    let members: Vec<AgentRecord> = records
        .into_iter()
        .filter(|record| !record.archived)
        .collect();
    let member_ids: HashSet<&AgentId> = members.iter().map(|record| &record.id).collect();
    let mut available: Vec<PopulateAgent> = store
        .all_agents()?
        .into_iter()
        .filter(|record| !record.archived && !member_ids.contains(&record.id))
        .map(|record| view_agent(&record))
        .collect();
    let mut members: Vec<PopulateAgent> = members.iter().map(view_agent).collect();
    sort_agents(&mut available);
    sort_agents(&mut members);
    Ok(PopulateSnapshot {
        lab,
        available,
        members,
        archived_members,
    })
}

/// Replaces the active membership set in one transaction.
///
/// The desired set must contain existing active profiles and exactly one
/// Tech Lead. Archived memberships are not part of the active set, so they are
/// left untouched.
pub fn save(
    store: &impl Directory,
    lab_id: &LabId,
    desired: &[AgentId],
) -> Result<PopulateReport, AppError> {
    store.lab(lab_id)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Laboratory,
        id: lab_id.to_string(),
    })?;

    let active: Vec<AgentRecord> = store
        .all_agents()?
        .into_iter()
        .filter(|record| !record.archived)
        .collect();
    let mut desired_ids: Vec<AgentId> = Vec::new();
    for id in desired {
        if !desired_ids.contains(id) {
            desired_ids.push(id.clone());
        }
    }
    if desired_ids.is_empty() {
        return Err(AppError::Validation {
            problem: "select at least one agent for the laboratory.".to_owned(),
            hint: "Add the Tech Lead and at least one other agent, then save.".to_owned(),
        });
    }
    for id in &desired_ids {
        if !active.iter().any(|record| &record.id == id) {
            return Err(AppError::Validation {
                problem: format!("agent `{id}` is not an active profile."),
                hint: "Reload the screen and choose an available agent.".to_owned(),
            });
        }
    }
    let leads = desired_ids
        .iter()
        .filter(|id| {
            active
                .iter()
                .find(|record| &record.id == *id)
                .and_then(|record| record.role().ok())
                .is_some_and(|role| role == Role::Lead)
        })
        .count();
    if leads != 1 {
        let problem = if leads == 0 {
            "the laboratory needs exactly one Tech Lead; none is selected.".to_owned()
        } else {
            format!("the laboratory needs exactly one Tech Lead; {leads} are selected.")
        };
        return Err(AppError::Validation {
            problem,
            hint: "Move agents between the panes so exactly one `lead` remains.".to_owned(),
        });
    }

    let current: HashSet<AgentId> = store
        .lab_agents(lab_id)?
        .into_iter()
        .filter(|record| !record.archived)
        .map(|record| record.id)
        .collect();
    let desired_set: HashSet<&AgentId> = desired_ids.iter().collect();
    let add: Vec<AgentId> = desired_ids
        .iter()
        .filter(|id| !current.contains(*id))
        .cloned()
        .collect();
    let remove: Vec<AgentId> = current
        .iter()
        .filter(|id| !desired_set.contains(id))
        .cloned()
        .collect();
    store.update_lab_memberships(lab_id, &add, &remove)?;
    Ok(PopulateReport {
        added: add.len(),
        removed: remove.len(),
        members: desired_ids.len(),
    })
}

fn view_agent(record: &AgentRecord) -> PopulateAgent {
    PopulateAgent {
        id: record.id.clone(),
        name: record.name.clone(),
        role: record
            .role()
            .map(|role| role.as_str().to_owned())
            .unwrap_or_else(|_| record.role.trim().to_owned()),
        description: record.description.clone(),
        skills: record
            .skills()
            .map(|tags| tags.iter().map(|tag| tag.as_str().to_owned()).collect())
            .unwrap_or_default(),
    }
}

fn sort_agents(agents: &mut [PopulateAgent]) {
    agents.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::app::directory::LabOverview;
    use crate::app::storage::StorageError;
    use crate::domain::agent::AgentProfile;
    use crate::domain::lab::Lab;

    #[derive(Default)]
    struct Store {
        lab: Option<LabOverview>,
        all: RefCell<Vec<AgentRecord>>,
        members: RefCell<Vec<AgentRecord>>,
        updates: RefCell<Vec<(Vec<AgentId>, Vec<AgentId>)>>,
    }

    impl Directory for Store {
        fn insert_lab(&self, _: &Lab) -> Result<(), StorageError> {
            unreachable!()
        }
        fn lab_exists(&self, _: &LabId) -> Result<bool, StorageError> {
            Ok(self.lab.is_some())
        }
        fn lab(&self, _: &LabId) -> Result<Option<LabOverview>, StorageError> {
            Ok(self.lab.clone())
        }
        fn labs(&self) -> Result<Vec<LabOverview>, StorageError> {
            Ok(self.lab.iter().cloned().collect())
        }
        fn lab_agents(&self, _: &LabId) -> Result<Vec<AgentRecord>, StorageError> {
            Ok(self.members.borrow().clone())
        }
        fn insert_agent(&self, _: &AgentProfile, _: &[LabId]) -> Result<(), StorageError> {
            unreachable!()
        }
        fn all_agents(&self) -> Result<Vec<AgentRecord>, StorageError> {
            Ok(self.all.borrow().clone())
        }
        fn update_lab_memberships(
            &self,
            _: &LabId,
            add: &[AgentId],
            remove: &[AgentId],
        ) -> Result<(), StorageError> {
            self.updates
                .borrow_mut()
                .push((add.to_vec(), remove.to_vec()));
            Ok(())
        }
        fn agent_exists(&self, _: &AgentId) -> Result<bool, StorageError> {
            Ok(true)
        }
        fn agent(&self, _: &AgentId) -> Result<Option<AgentRecord>, StorageError> {
            Ok(None)
        }
        fn agent_memberships(&self, _: &AgentId) -> Result<Vec<LabOverview>, StorageError> {
            Ok(Vec::new())
        }
    }

    fn lab_id() -> LabId {
        LabId::parse("a3f91c07").unwrap()
    }

    fn id(value: &str) -> AgentId {
        AgentId::parse(value).unwrap()
    }

    fn record(value: &str, name: &str, role: &str, archived: bool) -> AgentRecord {
        AgentRecord {
            id: id(value),
            name: name.to_owned(),
            role: role.to_owned(),
            description: "Works".to_owned(),
            skills_json: "[\"rust\"]".to_owned(),
            provider: "deepseek".to_owned(),
            model: "deepseek-chat".to_owned(),
            cv: None,
            archived,
        }
    }

    fn store() -> Store {
        let store = Store {
            lab: Some(LabOverview {
                id: lab_id(),
                name: "Payments".to_owned(),
                description: String::new(),
                agent_count: 2,
                project_count: 0,
            }),
            ..Store::default()
        };
        store.all.borrow_mut().extend([
            record("10000001", "Klaus", "lead", false),
            record("10000002", "Bram", "dev", false),
            record("10000003", "Nigel", "qa", false),
            record("10000004", "Old", "dev", true),
        ]);
        store.members.borrow_mut().extend([
            record("10000001", "Klaus", "lead", false),
            record("10000002", "Bram", "dev", false),
            record("10000004", "Old", "dev", true),
        ]);
        store
    }

    #[test]
    fn show_partitions_available_and_members_without_archived_profiles() {
        let snapshot = show(&store(), &lab_id()).unwrap();
        assert_eq!(snapshot.lab.name, "Payments");
        assert_eq!(snapshot.archived_members, 1);
        assert_eq!(
            snapshot
                .members
                .iter()
                .map(|agent| agent.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Bram", "Klaus"]
        );
        assert_eq!(
            snapshot
                .available
                .iter()
                .map(|agent| agent.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Nigel"]
        );
        assert!(snapshot.members[0].skills == vec!["rust"]);
    }

    #[test]
    fn saving_computes_the_membership_diff_in_one_call() {
        let store = store();
        let report = save(&store, &lab_id(), &[id("10000001"), id("10000003")]).unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.removed, 1);
        assert_eq!(report.members, 2);
        let updates = store.updates.borrow();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, vec![id("10000003")]);
        assert_eq!(updates[0].1, vec![id("10000002")]);
    }

    #[test]
    fn the_final_roster_needs_exactly_one_lead() {
        let store = store();
        let error = save(&store, &lab_id(), &[id("10000002"), id("10000003")]).unwrap_err();
        assert!(matches!(error, AppError::Validation { .. }), "{error:?}");

        store
            .all
            .borrow_mut()
            .push(record("10000009", "Other Lead", "lead", false));
        let error = save(&store, &lab_id(), &[id("10000001"), id("10000009")]).unwrap_err();
        assert!(matches!(error, AppError::Validation { .. }), "{error:?}");
        assert!(store.updates.borrow().is_empty());
    }

    #[test]
    fn an_empty_or_inactive_selection_is_rejected() {
        let store = store();
        assert!(matches!(
            save(&store, &lab_id(), &[]),
            Err(AppError::Validation { .. })
        ));
        assert!(matches!(
            save(&store, &lab_id(), &[id("10000004")]),
            Err(AppError::Validation { .. })
        ));
        assert!(matches!(
            save(&store, &lab_id(), &[id("10000001"), id("ffffffff")]),
            Err(AppError::Validation { .. })
        ));
        assert!(store.updates.borrow().is_empty());
    }

    #[test]
    fn an_unknown_laboratory_is_missing() {
        let store = Store::default();
        assert!(matches!(
            show(&store, &lab_id()),
            Err(AppError::NotFound { .. })
        ));
        assert!(matches!(
            save(&store, &lab_id(), &[id("10000001")]),
            Err(AppError::NotFound { .. })
        ));
    }
}
