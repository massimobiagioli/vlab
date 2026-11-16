//! Preset agent catalog validation (ADR-0052).
//!
//! A preset is a validated, reusable agent profile without an identifier or
//! provider binding. The loader supplies the identifier and the provider/model
//! from settings at creation time. All rules mirror the interactive wizard.

use crate::domain::agent::{
    AgentError, Description, DescriptionError, Role, RoleError, normalize_cv,
};
use crate::domain::name::{Name, NameError};
use crate::domain::skills::{self, SkillError, SkillTag};

/// Largest accepted roster.
pub const PRESET_MAX_AGENTS: usize = 50;

/// One validated agent profile from the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetAgent {
    pub name: Name,
    pub role: Role,
    pub description: Description,
    pub skills: Vec<SkillTag>,
    pub cv: Option<String>,
}

impl PresetAgent {
    /// Validates one catalog entry. `skills` are raw tags from the file.
    pub fn parse(
        name: &str,
        role: &str,
        description: &str,
        skills: &[String],
        cv: Option<String>,
    ) -> Result<Self, PresetAgentError> {
        let name = Name::parse(name)?;
        let role = Role::parse(role)?;
        let description = Description::parse(description)?;
        let skills = skills::parse_tags(skills.iter().map(String::as_str))?;
        if role.requires_skills() && skills.is_empty() {
            return Err(PresetAgentError::MissingSkills);
        }
        let cv = normalize_cv(cv)?;
        Ok(Self {
            name,
            role,
            description,
            skills,
            cv,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PresetAgentError {
    #[error("its name is invalid: {0}")]
    Name(#[from] NameError),
    #[error("its role is invalid: {0}")]
    Role(#[from] RoleError),
    #[error("its description is invalid: {0}")]
    Description(#[from] DescriptionError),
    #[error("its skills are invalid: {0}")]
    Skills(#[from] SkillError),
    #[error("this role needs at least one skill.")]
    MissingSkills,
    #[error("its CV is invalid: {0}")]
    Cv(#[from] AgentError),
}

/// A validated catalog with at least one agent and exactly one Tech Lead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetRoster {
    agents: Vec<PresetAgent>,
}

impl PresetRoster {
    pub fn new(agents: Vec<PresetAgent>) -> Result<Self, PresetRosterError> {
        if agents.is_empty() {
            return Err(PresetRosterError::Empty);
        }
        if agents.len() > PRESET_MAX_AGENTS {
            return Err(PresetRosterError::TooMany);
        }
        for (index, agent) in agents.iter().enumerate() {
            let duplicate = agents[..index].iter().any(|other| {
                other
                    .name
                    .as_str()
                    .eq_ignore_ascii_case(agent.name.as_str())
            });
            if duplicate {
                return Err(PresetRosterError::DuplicateName(
                    agent.name.as_str().to_owned(),
                ));
            }
        }
        let leads = agents
            .iter()
            .filter(|agent| agent.role == Role::Lead)
            .count();
        if leads != 1 {
            return Err(PresetRosterError::LeadCount);
        }
        Ok(Self { agents })
    }

    pub fn agents(&self) -> &[PresetAgent] {
        &self.agents
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PresetRosterError {
    #[error("a preset roster must contain at least one agent.")]
    Empty,
    #[error("a preset roster may contain at most {PRESET_MAX_AGENTS} agents.")]
    TooMany,
    #[error("the name `{0}` appears more than once.")]
    DuplicateName(String),
    #[error("a preset roster must contain exactly one `lead` agent.")]
    LeadCount,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(name: &str, role: &str, skills: &[&str]) -> PresetAgent {
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

    #[test]
    fn an_agent_uses_the_wizard_limits_and_requires_skills() {
        let lead = agent("Klaus", "lead", &[]);
        assert_eq!(lead.role, Role::Lead);
        let dev = agent("Bram", "dev", &["rust", "RUST", "api"]);
        assert_eq!(dev.skills.len(), 2);

        let missing = PresetAgent::parse("Bram", "dev", "Works", &[], None);
        assert_eq!(missing, Err(PresetAgentError::MissingSkills));

        let bad_role = PresetAgent::parse("Bram", "boss", "Works", &["rust".to_owned()], None);
        assert!(matches!(bad_role, Err(PresetAgentError::Role(_))));

        let bad_skill = PresetAgent::parse("Bram", "dev", "Works", &["two words".to_owned()], None);
        assert!(matches!(bad_skill, Err(PresetAgentError::Skills(_))));

        let bad_cv = PresetAgent::parse(
            "Bram",
            "dev",
            "Works",
            &["rust".to_owned()],
            Some("\0".to_owned()),
        );
        assert!(matches!(bad_cv, Err(PresetAgentError::Cv(_))));
    }

    #[test]
    fn a_roster_needs_one_lead_and_unique_names() {
        let roster = PresetRoster::new(vec![
            agent("Klaus", "lead", &[]),
            agent("Bram", "dev", &["rust"]),
        ])
        .unwrap();
        assert_eq!(roster.agents().len(), 2);

        assert_eq!(PresetRoster::new(Vec::new()), Err(PresetRosterError::Empty));
        assert_eq!(
            PresetRoster::new(vec![agent("Bram", "dev", &["rust"])]),
            Err(PresetRosterError::LeadCount)
        );
        assert_eq!(
            PresetRoster::new(vec![
                agent("Klaus", "lead", &[]),
                agent("Other", "lead", &[])
            ]),
            Err(PresetRosterError::LeadCount)
        );
        assert_eq!(
            PresetRoster::new(vec![
                agent("Klaus", "lead", &[]),
                agent("klaus", "dev", &["rust"])
            ]),
            Err(PresetRosterError::DuplicateName("klaus".to_owned()))
        );
    }

    #[test]
    fn the_roster_size_is_bounded() {
        let mut agents = vec![agent("Lead", "lead", &[])];
        for index in 0..PRESET_MAX_AGENTS {
            agents.push(agent(&format!("Agent {index}"), "dev", &["rust"]));
        }
        let error = PresetRoster::new(agents).unwrap_err();
        assert_eq!(error, PresetRosterError::TooMany);
        assert!(error.to_string().contains("50"), "{error}");
    }
}
