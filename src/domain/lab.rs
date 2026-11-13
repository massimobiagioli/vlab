//! Laboratory identity and membership invariants (specification §3.1).

use crate::domain::agent::Role;
use crate::domain::ids::LabId;
use crate::domain::name::Name;

/// Largest accepted laboratory description, in characters.
pub const LAB_DESCRIPTION_MAX_CHARS: usize = 2_000;

/// A trimmed, non-empty, single-line laboratory description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabDescription(String);

impl LabDescription {
    /// Returns `None` for an empty or whitespace-only value.
    pub fn parse(input: &str) -> Result<Option<Self>, LabDescriptionError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if trimmed.chars().count() > LAB_DESCRIPTION_MAX_CHARS {
            return Err(LabDescriptionError::TooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(LabDescriptionError::ControlCharacters);
        }
        Ok(Some(Self(trimmed.to_owned())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LabDescriptionError {
    #[error("a laboratory description may contain at most 2000 characters.")]
    TooLong,
    #[error("a laboratory description must not contain control characters.")]
    ControlCharacters,
}

/// A laboratory: a named workspace with an optional description. Memberships
/// are stored separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lab {
    pub id: LabId,
    pub name: Name,
    pub description: Option<LabDescription>,
}

impl Lab {
    pub fn new(id: LabId, name: Name) -> Self {
        Self {
            id,
            name,
            description: None,
        }
    }

    pub fn with_description(id: LabId, name: Name, description: Option<LabDescription>) -> Self {
        Self {
            id,
            name,
            description,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LabError {
    #[error("a laboratory may have at most one Tech Lead.")]
    SecondLead,
}

/// Rejects adding a `lead` to a laboratory that already has one.
///
/// `existing` holds the roles of the current members. A malformed role in
/// storage is not treated as a lead here; `--check-lab` reports it separately.
pub fn ensure_can_join(
    existing: impl IntoIterator<Item = Role>,
    candidate: Role,
) -> Result<(), LabError> {
    if candidate == Role::Lead && existing.into_iter().any(|role| role == Role::Lead) {
        return Err(LabError::SecondLead);
    }
    Ok(())
}

/// The number of members with the `lead` role.
pub fn lead_count(roles: impl IntoIterator<Item = Role>) -> usize {
    roles.into_iter().filter(|role| *role == Role::Lead).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laboratory_descriptions_are_optional_trimmed_and_bounded() {
        assert_eq!(LabDescription::parse("  ").unwrap(), None);
        assert_eq!(
            LabDescription::parse("  Payments platform  ")
                .unwrap()
                .unwrap()
                .as_str(),
            "Payments platform"
        );
        let too_long = "x".repeat(LAB_DESCRIPTION_MAX_CHARS + 1);
        assert_eq!(
            LabDescription::parse(&too_long),
            Err(LabDescriptionError::TooLong)
        );
        assert_eq!(
            LabDescription::parse("bad\u{7}"),
            Err(LabDescriptionError::ControlCharacters)
        );
    }

    #[test]
    fn a_laboratory_accepts_at_most_one_lead() {
        assert_eq!(ensure_can_join([], Role::Lead), Ok(()));
        assert_eq!(ensure_can_join([Role::Dev], Role::Lead), Ok(()));
        assert_eq!(
            ensure_can_join([Role::Lead, Role::Dev], Role::Lead),
            Err(LabError::SecondLead)
        );
        assert_eq!(ensure_can_join([Role::Lead], Role::Dev), Ok(()));
        assert_eq!(lead_count([Role::Lead, Role::Dev, Role::Lead]), 2);
        assert_eq!(lead_count([]), 0);
    }

    #[test]
    fn a_lab_carries_a_typed_identifier_and_name() {
        let lab = Lab::new(
            LabId::parse("a3f91c07").unwrap(),
            Name::parse("Payments Platform").unwrap(),
        );
        assert_eq!(lab.id.as_str(), "a3f91c07");
        assert_eq!(lab.name.as_str(), "Payments Platform");
    }
}
