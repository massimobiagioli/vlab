//! Destructive operations with bound confirmation support (specification
//! §6.4, §7.3, §8.3; A-08, A-15).

use crate::app::AppError;
use crate::app::directory::LabOverview;
use crate::app::storage::AppStateStore;
use crate::domain::ids::{AgentId, LabId, ProjectId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabDeletionPlan {
    pub id: LabId,
    pub name: String,
    pub projects: u64,
    pub memberships: u64,
    pub memories: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDeletionPlan {
    pub id: ProjectId,
    pub lab_id: LabId,
    pub name: String,
    pub requests: u64,
    pub stories: u64,
    pub memories: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDeletionPlan {
    pub id: AgentId,
    pub name: String,
    pub labs: Vec<LabOverview>,
    pub assigned_open_stories: u64,
    pub lead_labs: Vec<LabId>,
    pub memories: u64,
    pub archived: bool,
}

pub trait DeletionStore {
    fn lab_plan(&self, id: &LabId) -> Result<LabDeletionPlan, AppError>;
    fn delete_lab(&self, plan: &LabDeletionPlan) -> Result<(), AppError>;

    fn project_plan(&self, id: &ProjectId, lab: &LabId) -> Result<ProjectDeletionPlan, AppError>;
    fn delete_project(&self, plan: &ProjectDeletionPlan) -> Result<(), AppError>;

    fn agent_plan(&self, id: &AgentId) -> Result<AgentDeletionPlan, AppError>;
    fn delete_agent(&self, plan: &AgentDeletionPlan) -> Result<(), AppError>;
}

pub fn preflight_lab(
    store: &impl DeletionStore,
    raw_id: &str,
) -> Result<LabDeletionPlan, AppError> {
    let id = crate::app::labs::parse_lab_id(raw_id)?;
    store.lab_plan(&id)
}

pub fn preflight_project(
    store: &impl DeletionStore,
    state: &impl AppStateStore,
    id: &ProjectId,
) -> Result<ProjectDeletionPlan, AppError> {
    let lab = crate::app::context::active_lab(state)?.require()?;
    store.project_plan(id, &lab)
}

pub fn preflight_agent(
    store: &impl DeletionStore,
    raw_id: &str,
) -> Result<AgentDeletionPlan, AppError> {
    let id = crate::app::agents::parse_agent_id(raw_id)?;
    store.agent_plan(&id)
}

pub fn delete_lab(store: &impl DeletionStore, plan: &LabDeletionPlan) -> Result<(), AppError> {
    store.delete_lab(plan)
}

pub fn delete_project(
    store: &impl DeletionStore,
    plan: &ProjectDeletionPlan,
) -> Result<(), AppError> {
    store.delete_project(plan)
}

pub fn delete_agent(store: &impl DeletionStore, plan: &AgentDeletionPlan) -> Result<(), AppError> {
    store.delete_agent(plan)
}
