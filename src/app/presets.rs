//! Preset agent catalog loading (ADR-0052).
//!
//! The catalog contains validated profiles but no memberships; a laboratory
//! populates itself through the screen described in ADR-0053.

use std::collections::HashSet;

use crate::app::AppError;
use crate::app::directory::Directory;
use crate::domain::agent::AgentProfile;
use crate::domain::ids::{AgentId, UniqueIdError, unique_short_id};
use crate::domain::preset::PresetRoster;
use crate::domain::settings::Settings;

/// Result of one catalog load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetLoadReport {
    pub created: Vec<AgentProfile>,
    pub skipped: Vec<String>,
}

/// Creates every preset whose name is not already present. Existing profiles,
/// including archived ones, are never changed. All inserts share one
/// transaction through the directory port.
pub fn load(
    store: &impl Directory,
    roster: &PresetRoster,
    settings: &Settings,
) -> Result<PresetLoadReport, AppError> {
    let existing: HashSet<String> = store
        .all_agents()?
        .into_iter()
        .map(|record| record.name.trim().to_lowercase())
        .collect();

    let mut pending: Vec<(AgentProfile, Vec<crate::domain::ids::LabId>)> = Vec::new();
    let mut created = Vec::new();
    let mut skipped = Vec::new();
    let mut names = existing;

    for preset in roster.agents() {
        let key = preset.name.as_str().trim().to_lowercase();
        if names.contains(&key) {
            skipped.push(preset.name.as_str().to_owned());
            continue;
        }
        let generated = unique_short_id(|candidate| match AgentId::parse(candidate) {
            Some(id) => store.agent_exists(&id),
            None => Ok(true),
        })
        .map_err(map_unique)?;
        let id = AgentId::parse(&generated).ok_or(AppError::Internal)?;
        let profile = AgentProfile::new(
            id,
            preset.name.clone(),
            preset.role,
            preset.description.clone(),
            preset.skills.clone(),
            settings.provider,
            settings.model.clone(),
            preset.cv.clone(),
        )
        .map_err(|error| AppError::Validation {
            problem: format!("preset `{}` is invalid: {error}", preset.name),
            hint: "Fix the preset file and run the command again.".to_owned(),
        })?;
        names.insert(key);
        created.push(profile.clone());
        pending.push((profile, Vec::new()));
    }

    if !pending.is_empty() {
        store.insert_agents(&pending)?;
    }
    Ok(PresetLoadReport { created, skipped })
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

    use super::*;
    use crate::app::directory::{AgentRecord, LabOverview};
    use crate::app::storage::StorageError;
    use crate::domain::ids::LabId;
    use crate::domain::lab::Lab;
    use crate::domain::preset::PresetAgent;
    use crate::domain::settings::{ModelId, ProviderId};

    #[derive(Default)]
    struct Store {
        agents: RefCell<Vec<AgentRecord>>,
        inserts: RefCell<Vec<(AgentProfile, Vec<LabId>)>>,
    }

    impl Directory for Store {
        fn insert_lab(&self, _: &Lab) -> Result<(), StorageError> {
            unreachable!()
        }
        fn lab_exists(&self, _: &LabId) -> Result<bool, StorageError> {
            Ok(false)
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
        fn insert_agent(
            &self,
            profile: &AgentProfile,
            memberships: &[LabId],
        ) -> Result<(), StorageError> {
            self.inserts
                .borrow_mut()
                .push((profile.clone(), memberships.to_vec()));
            Ok(())
        }
        fn all_agents(&self) -> Result<Vec<AgentRecord>, StorageError> {
            Ok(self.agents.borrow().clone())
        }
        fn insert_agents(&self, agents: &[(AgentProfile, Vec<LabId>)]) -> Result<(), StorageError> {
            self.inserts.borrow_mut().extend(agents.iter().cloned());
            Ok(())
        }
        fn agent_exists(&self, id: &AgentId) -> Result<bool, StorageError> {
            Ok(self.agents.borrow().iter().any(|record| &record.id == id))
        }
        fn agent(&self, _: &AgentId) -> Result<Option<AgentRecord>, StorageError> {
            Ok(None)
        }
        fn agent_memberships(&self, _: &AgentId) -> Result<Vec<LabOverview>, StorageError> {
            Ok(Vec::new())
        }
    }

    fn settings() -> Settings {
        Settings::parse("deepseek", "deepseek-chat").unwrap()
    }

    fn record(id: &str, name: &str, role: &str, archived: bool) -> AgentRecord {
        AgentRecord {
            id: AgentId::parse(id).unwrap(),
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

    fn preset(name: &str, role: &str, skills: &[&str]) -> PresetAgent {
        PresetAgent::parse(
            name,
            role,
            "Works on the project",
            &skills
                .iter()
                .map(|tag| (*tag).to_owned())
                .collect::<Vec<_>>(),
            None,
        )
        .unwrap()
    }

    fn roster() -> PresetRoster {
        PresetRoster::new(vec![
            preset("Klaus Vandenberg", "lead", &[]),
            preset("Bram Kellerhof", "dev", &["rust"]),
            preset("Nigel Hoekstra", "dev", &["typescript"]),
        ])
        .unwrap()
    }

    #[test]
    fn loading_creates_profiles_without_memberships_and_skips_existing_names() {
        let store = Store::default();
        store
            .agents
            .borrow_mut()
            .push(record("10000001", "Bram Kellerhof", "dev", false));
        let report = load(&store, &roster(), &settings()).unwrap();
        assert_eq!(report.skipped, vec!["Bram Kellerhof"]);
        assert_eq!(report.created.len(), 2);
        let inserts = store.inserts.borrow();
        assert_eq!(inserts.len(), 2);
        assert!(
            inserts
                .iter()
                .all(|(_, memberships)| memberships.is_empty())
        );
        assert_eq!(inserts[0].0.provider, ProviderId::DeepSeek);
        assert_eq!(inserts[0].0.model, ModelId::parse("deepseek-chat").unwrap());
    }

    #[test]
    fn an_archived_profile_still_blocks_the_same_name() {
        let store = Store::default();
        store
            .agents
            .borrow_mut()
            .push(record("10000001", "klaus vandenberg", "lead", true));
        let report = load(&store, &roster(), &settings()).unwrap();
        assert_eq!(report.skipped, vec!["Klaus Vandenberg"]);
        assert_eq!(report.created.len(), 2);
    }

    #[test]
    fn a_second_load_is_idempotent() {
        let store = Store::default();
        let first = load(&store, &roster(), &settings()).unwrap();
        assert_eq!(first.created.len(), 3);
        // Simulate the persisted profiles for the next run.
        {
            let mut agents = store.agents.borrow_mut();
            for profile in &first.created {
                agents.push(record(
                    profile.id.as_str(),
                    profile.name.as_str(),
                    profile.role.as_str(),
                    false,
                ));
            }
        }
        let second = load(&store, &roster(), &settings()).unwrap();
        assert!(second.created.is_empty());
        assert_eq!(second.skipped.len(), 3);
    }
}
