//! Agent profiles and roles (specification §3.3, §7.2).
//!
//! The profile is pure data: no database annotations and no I/O. Provider and
//! model are copied from settings at creation time (specification §5.4) and
//! never re-read afterwards.

use crate::domain::ids::AgentId;
use crate::domain::name::{Name, NameError};
use crate::domain::settings::{ModelId, ProviderId};
use crate::domain::skills::{SkillError, SkillTag};

/// Longest accepted description, in characters after trimming.
pub const DESCRIPTION_MAX_CHARS: usize = 2_000;

/// Largest accepted CV/profile text, in bytes.
pub const CV_MAX_BYTES: usize = 64 * 1024;

/// Supported agent roles. Parsing is case-insensitive and trims surrounding
/// whitespace; display names are canonical lowercase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Lead,
    Dev,
    Analyst,
    Qa,
}

impl Role {
    pub fn parse(input: &str) -> Result<Self, RoleError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "lead" => Ok(Self::Lead),
            "dev" => Ok(Self::Dev),
            "analyst" => Ok(Self::Analyst),
            "qa" => Ok(Self::Qa),
            _ => Err(RoleError),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lead => "lead",
            Self::Dev => "dev",
            Self::Analyst => "analyst",
            Self::Qa => "qa",
        }
    }

    /// Whether `--check-lab` requires at least one skill for this role (A-07).
    pub fn requires_skills(self) -> bool {
        !matches!(self, Self::Lead)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a role must be `lead`, `dev`, `analyst`, or `qa`.")]
pub struct RoleError;

/// A trimmed, non-empty, single-line description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description(String);

impl Description {
    pub fn parse(input: &str) -> Result<Self, DescriptionError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(DescriptionError::Empty);
        }
        if trimmed.chars().count() > DESCRIPTION_MAX_CHARS {
            return Err(DescriptionError::TooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(DescriptionError::ControlCharacters);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DescriptionError {
    #[error("a description is required.")]
    Empty,
    #[error("a description may contain at most 2000 characters.")]
    TooLong,
    #[error("a description must not contain control characters.")]
    ControlCharacters,
}

/// A complete, validated agent profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProfile {
    pub id: AgentId,
    pub name: Name,
    pub role: Role,
    pub description: Description,
    pub skills: Vec<SkillTag>,
    pub provider: ProviderId,
    pub model: ModelId,
    pub cv: Option<String>,
}

impl AgentProfile {
    /// Validates the remaining fields and returns a profile.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: AgentId,
        name: Name,
        role: Role,
        description: Description,
        skills: Vec<SkillTag>,
        provider: ProviderId,
        model: ModelId,
        cv: Option<String>,
    ) -> Result<Self, AgentError> {
        let cv = match cv {
            Some(text) if text.trim().is_empty() => None,
            Some(text) => {
                if text.len() > CV_MAX_BYTES {
                    return Err(AgentError::CvTooLarge);
                }
                if text.contains('\0') {
                    return Err(AgentError::CvNull);
                }
                Some(text)
            }
            None => None,
        };
        Ok(Self {
            id,
            name,
            role,
            description,
            skills,
            provider,
            model,
            cv,
        })
    }

    pub fn has_skills(&self) -> bool {
        !self.skills.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AgentError {
    #[error(transparent)]
    Name(#[from] NameError),
    #[error(transparent)]
    Description(#[from] DescriptionError),
    #[error(transparent)]
    Skills(#[from] SkillError),
    #[error("a CV may contain at most 64 KiB.")]
    CvTooLarge,
    #[error("a CV must not contain NUL characters.")]
    CvNull,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::skills::parse_list;

    fn profile(role: Role, skills: Vec<SkillTag>) -> AgentProfile {
        AgentProfile::new(
            AgentId::generate(),
            Name::parse("Ada").unwrap(),
            role,
            Description::parse("Backend engineer").unwrap(),
            skills,
            ProviderId::DeepSeek,
            ModelId::parse("deepseek-chat").unwrap(),
            None,
        )
        .unwrap()
    }

    #[test]
    fn roles_parse_case_insensitively_and_report_skill_requirements() {
        for (input, expected) in [
            ("lead", Role::Lead),
            (" LEAD ", Role::Lead),
            ("Dev", Role::Dev),
            ("analyst", Role::Analyst),
            ("qa", Role::Qa),
        ] {
            assert_eq!(Role::parse(input), Ok(expected), "{input:?}");
            assert_eq!(Role::parse(expected.as_str()), Ok(expected));
        }
        for bad in ["", "boss", "developer", "le ad"] {
            assert_eq!(Role::parse(bad), Err(RoleError), "{bad:?}");
        }
        assert!(!Role::Lead.requires_skills());
        for role in [Role::Dev, Role::Analyst, Role::Qa] {
            assert!(role.requires_skills());
        }
    }

    #[test]
    fn descriptions_are_trimmed_and_bounded() {
        assert_eq!(
            Description::parse("  Coordinates planning  ")
                .unwrap()
                .as_str(),
            "Coordinates planning"
        );
        assert_eq!(Description::parse("   "), Err(DescriptionError::Empty));
        assert_eq!(
            Description::parse(&"a".repeat(DESCRIPTION_MAX_CHARS + 1)),
            Err(DescriptionError::TooLong)
        );
        assert_eq!(
            Description::parse("line\nbreak"),
            Err(DescriptionError::ControlCharacters)
        );
    }

    #[test]
    fn a_profile_accepts_optional_cv_text_within_the_limit() {
        let profile = profile(Role::Qa, parse_list("testing").unwrap());
        assert_eq!(profile.role, Role::Qa);
        assert!(profile.has_skills());
        assert_eq!(profile.cv, None);

        let cv = "x".repeat(CV_MAX_BYTES);
        let with_cv = AgentProfile::new(
            AgentId::generate(),
            Name::parse("Ada").unwrap(),
            Role::Lead,
            Description::parse("Leads").unwrap(),
            Vec::new(),
            ProviderId::DeepSeek,
            ModelId::parse("m").unwrap(),
            Some(cv),
        )
        .unwrap();
        assert!(with_cv.cv.is_some());

        let too_large = AgentProfile::new(
            AgentId::generate(),
            Name::parse("Ada").unwrap(),
            Role::Lead,
            Description::parse("Leads").unwrap(),
            Vec::new(),
            ProviderId::DeepSeek,
            ModelId::parse("m").unwrap(),
            Some("x".repeat(CV_MAX_BYTES + 1)),
        );
        assert_eq!(too_large, Err(AgentError::CvTooLarge));
    }

    #[test]
    fn empty_cv_text_is_stored_as_absent() {
        let profile = AgentProfile::new(
            AgentId::generate(),
            Name::parse("Ada").unwrap(),
            Role::Lead,
            Description::parse("Leads").unwrap(),
            Vec::new(),
            ProviderId::DeepSeek,
            ModelId::parse("m").unwrap(),
            Some("  \n ".to_owned()),
        )
        .unwrap();
        assert_eq!(profile.cv, None);
    }
}
