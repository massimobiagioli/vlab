//! The laboratory and agent directory port (WP-3).
//!
//! The application declares the queries it needs; `infra::db` implements them
//! with SQLite. Records keep the stored strings as-is so that `--check-lab`
//! can report malformed values instead of failing the whole command.

use crate::app::storage::StorageError;
use crate::domain::agent::{
    AgentError, AgentProfile, Description, DescriptionError, Role, RoleError,
};
use crate::domain::ids::{AgentId, LabId, ProjectId};
use crate::domain::name::{Name, NameError};
use crate::domain::settings::{ModelId, ProviderId, SettingsError};
use crate::domain::skills::{self, SkillError, SkillTag};

/// One laboratory with its active-member count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabOverview {
    pub id: LabId,
    pub name: String,
    pub agent_count: u64,
    pub project_count: u64,
}

/// A stored agent row, before field-level validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRecord {
    pub id: AgentId,
    pub name: String,
    pub role: String,
    pub description: String,
    pub skills_json: String,
    pub provider: String,
    pub model: String,
    pub cv: Option<String>,
    pub archived: bool,
}

/// Which stored field could not be turned into a domain value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AgentRecordError {
    #[error("its name is invalid")]
    Name,
    #[error("its role is invalid")]
    Role,
    #[error("its description is invalid")]
    Description,
    #[error("its skills are invalid")]
    Skills,
    #[error("its provider is invalid")]
    Provider,
    #[error("its model is invalid")]
    Model,
    #[error("its CV is invalid")]
    Cv,
}

impl AgentRecord {
    pub fn role(&self) -> Result<Role, RoleError> {
        Role::parse(&self.role)
    }

    pub fn skills(&self) -> Result<Vec<SkillTag>, SkillError> {
        skills::decode(&self.skills_json)
    }

    /// Converts the row into a validated profile or names the first bad field.
    pub fn profile(&self) -> Result<AgentProfile, AgentRecordError> {
        let name = Name::parse(&self.name).map_err(map_name)?;
        let role = self.role().map_err(|_| AgentRecordError::Role)?;
        let description = Description::parse(&self.description).map_err(map_description)?;
        let tags = self.skills().map_err(|_| AgentRecordError::Skills)?;
        let provider = ProviderId::parse(&self.provider).map_err(map_provider)?;
        let model = ModelId::parse(&self.model).map_err(map_model)?;
        AgentProfile::new(
            self.id.clone(),
            name,
            role,
            description,
            tags,
            provider,
            model,
            self.cv.clone(),
        )
        .map_err(map_agent)
    }
}

fn map_name(_: NameError) -> AgentRecordError {
    AgentRecordError::Name
}

fn map_description(_: DescriptionError) -> AgentRecordError {
    AgentRecordError::Description
}

fn map_provider(_: SettingsError) -> AgentRecordError {
    AgentRecordError::Provider
}

fn map_model(_: SettingsError) -> AgentRecordError {
    AgentRecordError::Model
}

fn map_agent(_: AgentError) -> AgentRecordError {
    AgentRecordError::Cv
}

/// Reads and writes laboratories, agent profiles, and memberships.
pub trait Directory {
    fn insert_lab(&self, lab: &crate::domain::lab::Lab) -> Result<(), StorageError>;

    fn lab_exists(&self, id: &LabId) -> Result<bool, StorageError>;

    fn lab(&self, id: &LabId) -> Result<Option<LabOverview>, StorageError>;

    fn labs(&self) -> Result<Vec<LabOverview>, StorageError>;

    /// All members of `lab`, including archived profiles.
    fn lab_agents(&self, lab: &LabId) -> Result<Vec<AgentRecord>, StorageError>;

    /// Inserts the profile and every membership in one transaction.
    fn insert_agent(
        &self,
        profile: &AgentProfile,
        memberships: &[LabId],
    ) -> Result<(), StorageError>;

    fn agent_exists(&self, id: &AgentId) -> Result<bool, StorageError>;

    fn agent(&self, id: &AgentId) -> Result<Option<AgentRecord>, StorageError>;

    /// The laboratories the agent belongs to, with their member counts.
    fn agent_memberships(&self, id: &AgentId) -> Result<Vec<LabOverview>, StorageError>;

    // Optional metrics. Defaults keep test doubles small; the SQLite adapter
    // overrides them with real counts.
    fn agent_project_count(&self, _: &AgentId) -> Result<u64, StorageError> {
        Ok(0)
    }

    fn agent_open_story_count(&self, _: &AgentId) -> Result<u64, StorageError> {
        Ok(0)
    }

    fn agent_assigned_story_count(&self, _: &AgentId) -> Result<u64, StorageError> {
        Ok(0)
    }

    fn agent_completed_story_count(&self, _: &AgentId) -> Result<u64, StorageError> {
        Ok(0)
    }

    fn agent_memory_count(&self, _: &AgentId) -> Result<u64, StorageError> {
        Ok(0)
    }

    fn agent_project_names(&self, _: &AgentId) -> Result<Vec<(ProjectId, String)>, StorageError> {
        Ok(Vec::new())
    }
}
