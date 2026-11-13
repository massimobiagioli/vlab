//! Read-only laboratory floor read model (ADR-0045).
//!
//! The floor is an overview of stored assignments, not a live simulation. It
//! groups active agents by project, adds the planning `whole lab` fallback,
//! places unassigned agents in a lounge, and identifies the Tech Lead.

use std::collections::{HashMap, HashSet};

use crate::app::AppError;
use crate::app::directory::{AgentRecord, Directory, LabOverview};
use crate::app::projects::ProjectStore;
use crate::app::storage::AppStateStore;
use crate::domain::agent::Role;
use crate::domain::ids::{AgentId, ProjectId};

/// At or above this many open assigned stories an agent isolates at a focus
/// desk instead of joining the project huddle.
pub const FOCUS_OPEN_STORIES: u64 = 2;

/// One active agent as shown on the floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloorAgent {
    pub id: AgentId,
    pub name: String,
    pub role: String,
    pub skills: Vec<String>,
    pub open_stories: u64,
    /// Projects whose pod includes this agent (explicit or whole-lab).
    pub projects: Vec<ProjectId>,
}

impl FloorAgent {
    /// An agent with no open assigned story is idle and shows `(zzz)`.
    pub fn is_idle(&self) -> bool {
        self.open_stories == 0
    }

    /// An agent with several open stories isolates at a focus desk.
    pub fn is_focused(&self) -> bool {
        self.open_stories >= FOCUS_OPEN_STORIES
    }

    /// An agent with exactly one open story works in the project huddle.
    pub fn is_working(&self) -> bool {
        self.open_stories > 0 && !self.is_focused()
    }
}

/// Two working agents of different roles who exchange information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangePair {
    pub first: AgentId,
    pub second: AgentId,
}

/// One project table, or the lounge when `project_id` is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloorPod {
    pub project_id: Option<ProjectId>,
    pub name: String,
    pub members: Vec<AgentId>,
    /// The project has no explicit roster, so the whole active laboratory
    /// works on it (planning semantics, ADR-0034).
    pub whole_lab: bool,
}

/// Everything the floor screen renders, computed from stored data only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Floor {
    pub lab: LabOverview,
    pub lead: Option<AgentId>,
    pub agents: Vec<FloorAgent>,
    pub pods: Vec<FloorPod>,
    /// Active members are shown; archived members are counted and hidden.
    pub archived: usize,
}

impl Floor {
    pub fn agent(&self, id: &AgentId) -> Option<&FloorAgent> {
        self.agents.iter().find(|agent| &agent.id == id)
    }

    pub fn selected(&self, index: usize) -> Option<&FloorAgent> {
        self.agents.get(index)
    }

    pub fn project_pods(&self) -> impl Iterator<Item = &FloorPod> {
        self.pods.iter().filter(|pod| pod.project_id.is_some())
    }

    /// Pairs the working, non-lead members of `pod` whose roles differ.
    ///
    /// The order is the stored member order, so the scene is deterministic.
    /// Focused and idle agents are not paired; a focused agent sits at an
    /// isolated desk, and an idle agent is asleep.
    pub fn exchange_pairs(&self, pod: &FloorPod) -> Vec<ExchangePair> {
        let lead = self.lead.as_ref();
        let working: Vec<&FloorAgent> = pod
            .members
            .iter()
            .filter(|id| Some(*id) != lead)
            .filter_map(|id| self.agent(id))
            .filter(|agent| agent.is_working())
            .collect();
        let mut used = vec![false; working.len()];
        let mut pairs = Vec::new();
        for first in 0..working.len() {
            if used[first] {
                continue;
            }
            let role = Role::parse(&working[first].role).ok();
            let partner = (first + 1..working.len())
                .find(|index| !used[*index] && Role::parse(&working[*index].role).ok() != role);
            if let Some(second) = partner {
                pairs.push(ExchangePair {
                    first: working[first].id.clone(),
                    second: working[second].id.clone(),
                });
                used[first] = true;
                used[second] = true;
            }
        }
        pairs
    }

    /// Focused agents outside the Tech Lead desk, in scene order.
    pub fn focused_agents(&self) -> impl Iterator<Item = &FloorAgent> {
        let lead = self.lead.as_ref();
        self.agents
            .iter()
            .filter(move |agent| agent.is_focused() && Some(&agent.id) != lead)
    }
}

/// Builds the floor for the active laboratory.
///
/// Archived agents never appear. A project with no explicit assignments uses
/// the whole active laboratory, exactly like planning. The first active lead
/// is the Tech Lead. No writes and no provider calls happen here.
pub fn show(
    store: &(impl Directory + ProjectStore),
    state: &impl AppStateStore,
) -> Result<Floor, AppError> {
    let lab_id = crate::app::context::active_lab(state)?.require()?;
    let lab = store.lab(&lab_id)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Laboratory,
        id: lab_id.to_string(),
    })?;

    let records = store.lab_agents(&lab_id)?;
    let archived = records.iter().filter(|record| record.archived).count();
    let active: Vec<AgentRecord> = records
        .into_iter()
        .filter(|record| !record.archived)
        .collect();
    let active_ids: Vec<AgentId> = active.iter().map(|record| record.id.clone()).collect();
    let active_set: HashSet<&AgentId> = active_ids.iter().collect();
    let lead = active
        .iter()
        .find(|record| record.role().is_ok_and(|role| role == Role::Lead))
        .map(|record| record.id.clone());

    let mut pods = Vec::new();
    for project in store.projects(&lab_id)? {
        let assigned = store.project_agent_ids(&project.id)?;
        let whole_lab = assigned.is_empty();
        let members: Vec<AgentId> = if whole_lab {
            active_ids.clone()
        } else {
            assigned
                .into_iter()
                .filter(|id| active_set.contains(id))
                .collect()
        };
        pods.push(FloorPod {
            project_id: Some(project.id),
            name: project.name,
            members,
            whole_lab,
        });
    }

    let mut assigned: HashSet<&AgentId> = HashSet::new();
    for pod in &pods {
        assigned.extend(pod.members.iter());
    }
    let lounge: Vec<AgentId> = active_ids
        .iter()
        .filter(|id| !assigned.contains(*id) && Some(*id) != lead.as_ref())
        .cloned()
        .collect();
    if !lounge.is_empty() {
        pods.push(FloorPod {
            project_id: None,
            name: "Lounge".to_owned(),
            members: lounge,
            whole_lab: false,
        });
    }

    let by_id: HashMap<&AgentId, &AgentRecord> =
        active.iter().map(|record| (&record.id, record)).collect();
    let mut ordered: Vec<AgentId> = Vec::new();
    if let Some(lead) = &lead {
        ordered.push(lead.clone());
    }
    for pod in &pods {
        for id in &pod.members {
            if Some(id) != lead.as_ref() && !ordered.contains(id) {
                ordered.push(id.clone());
            }
        }
    }
    for id in &active_ids {
        if !ordered.contains(id) {
            ordered.push(id.clone());
        }
    }

    let mut agents = Vec::with_capacity(ordered.len());
    for id in ordered {
        let Some(record) = by_id.get(&id) else {
            continue;
        };
        let projects = pods
            .iter()
            .filter(|pod| pod.project_id.is_some() && pod.members.contains(&id))
            .filter_map(|pod| pod.project_id.clone())
            .collect();
        agents.push(FloorAgent {
            id: id.clone(),
            name: record.name.clone(),
            role: record
                .role()
                .map(|role| role.as_str().to_owned())
                .unwrap_or_else(|_| record.role.trim().to_owned()),
            skills: record
                .skills()
                .map(|tags| tags.iter().map(|tag| tag.as_str().to_owned()).collect())
                .unwrap_or_default(),
            open_stories: store.agent_open_story_count(&id)?,
            projects,
        });
    }

    Ok(Floor {
        lab,
        lead,
        agents,
        pods,
        archived,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;
    use crate::app::directory::LabOverview;
    use crate::app::projects::ProjectOverview;
    use crate::app::storage::{AppStateKey, StorageError};
    use crate::domain::ids::LabId;
    use crate::domain::lab::Lab;
    use crate::domain::project::Project;

    #[derive(Default)]
    struct Fake {
        labs: RefCell<HashMap<LabId, LabOverview>>,
        agents: RefCell<HashMap<LabId, Vec<AgentRecord>>>,
        projects: RefCell<HashMap<LabId, Vec<ProjectOverview>>>,
        assignments: RefCell<HashMap<ProjectId, Vec<AgentId>>>,
        open: RefCell<HashMap<AgentId, u64>>,
        active: RefCell<HashMap<&'static str, String>>,
    }

    impl AppStateStore for Fake {
        fn get(&self, key: AppStateKey) -> Result<Option<String>, StorageError> {
            Ok(self.active.borrow().get(key.as_str()).cloned())
        }
        fn set(&self, key: AppStateKey, value: &str) -> Result<(), StorageError> {
            self.active
                .borrow_mut()
                .insert(key.as_str(), value.to_owned());
            Ok(())
        }
        fn clear(&self, key: AppStateKey) -> Result<(), StorageError> {
            self.active.borrow_mut().remove(key.as_str());
            Ok(())
        }
    }

    impl Directory for Fake {
        fn insert_lab(&self, _: &Lab) -> Result<(), StorageError> {
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
            Ok(self.agents.borrow().get(lab).cloned().unwrap_or_default())
        }
        fn insert_agent(
            &self,
            _: &crate::domain::agent::AgentProfile,
            _: &[LabId],
        ) -> Result<(), StorageError> {
            unreachable!()
        }
        fn agent_exists(&self, id: &AgentId) -> Result<bool, StorageError> {
            Ok(self
                .agents
                .borrow()
                .values()
                .flatten()
                .any(|record| &record.id == id))
        }
        fn agent(&self, id: &AgentId) -> Result<Option<AgentRecord>, StorageError> {
            Ok(self
                .agents
                .borrow()
                .values()
                .flatten()
                .find(|record| &record.id == id)
                .cloned())
        }
        fn agent_memberships(&self, _: &AgentId) -> Result<Vec<LabOverview>, StorageError> {
            Ok(Vec::new())
        }
        fn agent_open_story_count(&self, id: &AgentId) -> Result<u64, StorageError> {
            Ok(self.open.borrow().get(id).copied().unwrap_or(0))
        }
    }

    impl ProjectStore for Fake {
        fn insert_project(&self, _: &Project) -> Result<(), StorageError> {
            unreachable!()
        }
        fn project(
            &self,
            _: &ProjectId,
        ) -> Result<Option<(ProjectId, LabId, String, Option<String>)>, StorageError> {
            Ok(None)
        }
        fn projects(&self, lab: &LabId) -> Result<Vec<ProjectOverview>, StorageError> {
            Ok(self.projects.borrow().get(lab).cloned().unwrap_or_default())
        }
        fn project_in_lab(
            &self,
            _: &ProjectId,
            _: &LabId,
        ) -> Result<Option<ProjectOverview>, StorageError> {
            Ok(None)
        }
        fn project_agent_ids(&self, id: &ProjectId) -> Result<Vec<AgentId>, StorageError> {
            Ok(self
                .assignments
                .borrow()
                .get(id)
                .cloned()
                .unwrap_or_default())
        }
    }

    fn id(value: &str) -> AgentId {
        AgentId::parse(value).unwrap()
    }

    fn lab_id() -> LabId {
        LabId::parse("a3f91c07").unwrap()
    }

    fn project_id(value: &str) -> ProjectId {
        ProjectId::parse(value).unwrap()
    }

    fn agent(id: &str, name: &str, role: &str, archived: bool) -> AgentRecord {
        AgentRecord {
            id: super::tests::id(id),
            name: name.to_owned(),
            role: role.to_owned(),
            description: String::new(),
            skills_json: "[\"rust\"]".to_owned(),
            provider: "deepseek".to_owned(),
            model: "deepseek-flash".to_owned(),
            cv: None,
            archived,
        }
    }

    fn project(id: &str, name: &str) -> ProjectOverview {
        ProjectOverview {
            id: project_id(id),
            name: name.to_owned(),
            github_source: None,
            request_count: 0,
            story_count: 0,
            todo: 0,
            in_progress: 0,
            in_review: 0,
            blocked: 0,
            done: 0,
        }
    }

    fn fake() -> Fake {
        let fake = Fake::default();
        fake.labs.borrow_mut().insert(
            lab_id(),
            LabOverview {
                id: lab_id(),
                name: "Demo Lab".to_owned(),
                description: String::new(),
                agent_count: 4,
                project_count: 2,
            },
        );
        fake.active
            .borrow_mut()
            .insert(AppStateKey::ActiveLabId.as_str(), lab_id().to_string());
        fake.agents.borrow_mut().insert(
            lab_id(),
            vec![
                agent("10000001", "Ada", "lead", false),
                agent("10000002", "Bea", "dev", false),
                agent("10000003", "Cid", "qa", false),
                agent("10000004", "Old", "dev", true),
            ],
        );
        fake.projects.borrow_mut().insert(
            lab_id(),
            vec![project("20000001", "Portal"), project("20000002", "Mobile")],
        );
        fake.assignments.borrow_mut().insert(
            project_id("20000001"),
            vec![
                id("10000002"),
                id("10000003"),
                id("10000004"), // archived: filtered out
            ],
        );
        fake.open.borrow_mut().insert(id("10000002"), 2);
        fake.open.borrow_mut().insert(id("10000003"), 0);
        fake
    }

    #[test]
    fn agents_are_grouped_by_project_and_the_lead_stays_at_the_desk() {
        let floor = show(&fake(), &fake()).unwrap();
        assert_eq!(floor.lab.name, "Demo Lab");
        assert_eq!(floor.lead, Some(id("10000001")));
        assert_eq!(floor.archived, 1);
        assert_eq!(floor.agents.len(), 3);
        assert_eq!(floor.agents[0].id, id("10000001"));
        assert_eq!(floor.pods[0].members, vec![id("10000002"), id("10000003")]);
        assert!(!floor.pods[0].whole_lab);
        // Mobile has no explicit roster, so the whole active lab works on it.
        assert!(floor.pods[1].whole_lab);
        assert_eq!(floor.pods[1].members.len(), 3);
        // The lead is not repeated in the lounge.
        assert_eq!(floor.pods.len(), 2);
        assert_eq!(floor.agents[1].id, id("10000002"));
        assert_eq!(floor.agents[1].open_stories, 2);
        assert!(!floor.agents[1].is_idle());
        assert!(floor.agents[2].is_idle());
    }

    #[test]
    fn unassigned_agents_go_to_the_lounge() {
        let fake = fake();
        fake.projects.borrow_mut().get_mut(&lab_id()).unwrap().pop();
        fake.assignments
            .borrow_mut()
            .insert(project_id("20000001"), vec![id("10000002")]);
        let floor = show(&fake, &fake).unwrap();
        assert_eq!(floor.pods.len(), 2);
        assert_eq!(floor.pods[1].project_id, None);
        assert_eq!(floor.pods[1].name, "Lounge");
        assert_eq!(floor.pods[1].members, vec![id("10000003")]);
        let cid = floor.agent(&id("10000003")).unwrap();
        assert!(cid.projects.is_empty());
    }

    #[test]
    fn archived_agents_are_counted_but_never_rendered() {
        let floor = show(&fake(), &fake()).unwrap();
        assert!(floor.agents.iter().all(|agent| agent.id != id("10000004")));
        assert_eq!(floor.archived, 1);
    }

    #[test]
    fn malformed_stored_roles_and_skills_do_not_fail_the_floor() {
        let fake = fake();
        {
            let mut agents = fake.agents.borrow_mut();
            let members = agents.get_mut(&lab_id()).unwrap();
            members[1].role = "wizard".to_owned();
            members[1].skills_json = "not json".to_owned();
        }
        let floor = show(&fake, &fake).unwrap();
        let bea = floor.agent(&id("10000002")).unwrap();
        assert_eq!(bea.role, "wizard");
        assert!(bea.skills.is_empty());
    }

    #[test]
    fn no_active_laboratory_is_a_context_error() {
        let fake = Fake::default();
        let error = show(&fake, &fake).unwrap_err();
        assert!(matches!(error, AppError::NoActiveLab));
    }

    #[test]
    fn working_agents_with_different_roles_form_a_conversation_pair() {
        let fake = fake();
        *fake.open.borrow_mut().get_mut(&id("10000002")).unwrap() = 1;
        *fake.open.borrow_mut().get_mut(&id("10000003")).unwrap() = 1;
        let floor = show(&fake, &fake).unwrap();
        let portal = floor.project_pods().next().unwrap();
        assert_eq!(
            floor.exchange_pairs(portal),
            vec![ExchangePair {
                first: id("10000002"),
                second: id("10000003"),
            }]
        );
    }

    #[test]
    fn idle_focused_or_same_role_agents_do_not_converse() {
        let fake = fake();
        *fake.open.borrow_mut().get_mut(&id("10000002")).unwrap() = 1;
        // Cid is idle.
        let floor = show(&fake, &fake).unwrap();
        let portal = floor.project_pods().next().unwrap();
        assert!(floor.exchange_pairs(portal).is_empty());

        // Cid is focused.
        *fake.open.borrow_mut().get_mut(&id("10000003")).unwrap() = 2;
        let floor = show(&fake, &fake).unwrap();
        let portal = floor.project_pods().next().unwrap();
        assert!(floor.exchange_pairs(portal).is_empty());

        // Both work, but they share a role.
        *fake.open.borrow_mut().get_mut(&id("10000003")).unwrap() = 1;
        fake.agents.borrow_mut().get_mut(&lab_id()).unwrap()[2].role = "dev".to_owned();
        let floor = show(&fake, &fake).unwrap();
        let portal = floor.project_pods().next().unwrap();
        assert!(floor.exchange_pairs(portal).is_empty());
    }

    #[test]
    fn focused_agents_isolate_but_the_lead_stays_at_the_desk() {
        let fake = fake();
        fake.open.borrow_mut().insert(id("10000001"), 5);
        let floor = show(&fake, &fake).unwrap();
        let bea = floor.agent(&id("10000002")).unwrap();
        assert!(bea.is_focused());
        assert!(!bea.is_working());
        assert_eq!(
            floor
                .focused_agents()
                .map(|agent| agent.id.clone())
                .collect::<Vec<_>>(),
            vec![id("10000002")]
        );
        // The lead is excluded from the focus corner and uses the desk.
        assert!(
            floor
                .focused_agents()
                .all(|agent| agent.id != id("10000001"))
        );
    }

    #[test]
    fn project_membership_is_deduplicated_in_the_agent_view() {
        let floor = show(&fake(), &fake()).unwrap();
        let bea = floor.agent(&id("10000002")).unwrap();
        // Portal is explicit; Mobile is whole-lab, so Bea works on both.
        assert_eq!(
            bea.projects,
            vec![project_id("20000001"), project_id("20000002")]
        );
    }
}
