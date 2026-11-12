//! Scoped memories (specification §3.5, ADR-0022).
//!
//! Memories are user-authored, immutable text attached to exactly one
//! laboratory, project, or agent. The model never writes one; they are
//! untrusted context and are never executed.

use crate::domain::ids::{AgentId, LabId, MemoryId, ProjectId};

/// Largest accepted memory, in characters after trimming.
pub const MEMORY_MAX_CHARS: usize = 2_000;

/// Maximum memories per scope.
pub const MEMORY_MAX_PER_SCOPE: usize = 20;

/// Total memory text included in one prompt, in UTF-8 bytes.
pub const MEMORY_CONTEXT_MAX_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryScope {
    Lab(LabId),
    Project(ProjectId),
    Agent(AgentId),
}

impl MemoryScope {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Lab(_) => "lab",
            Self::Project(_) => "project",
            Self::Agent(_) => "agent",
        }
    }

    pub fn owner_id(&self) -> &str {
        match self {
            Self::Lab(id) => id.as_str(),
            Self::Project(id) => id.as_str(),
            Self::Agent(id) => id.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memory {
    pub id: MemoryId,
    pub scope: MemoryScope,
    pub content: String,
}

impl Memory {
    pub fn new(id: MemoryId, scope: MemoryScope, content: &str) -> Result<Self, MemoryError> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err(MemoryError::Empty);
        }
        if trimmed.chars().count() > MEMORY_MAX_CHARS {
            return Err(MemoryError::TooLong);
        }
        if trimmed
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(MemoryError::ControlCharacters);
        }
        Ok(Self {
            id,
            scope,
            content: trimmed.to_owned(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MemoryError {
    #[error("a memory is required.")]
    Empty,
    #[error("a memory may contain at most 2000 characters.")]
    TooLong,
    #[error("a memory must not contain control characters other than newlines or tabs.")]
    ControlCharacters,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> MemoryScope {
        MemoryScope::Lab(LabId::parse("a3f91c07").unwrap())
    }

    #[test]
    fn memories_are_trimmed_and_bounded() {
        let memory = Memory::new(
            MemoryId::parse("0b1c2d3e").unwrap(),
            scope(),
            "  Prefer small pull requests.\nKeep tests green.\t ",
        )
        .unwrap();
        assert_eq!(
            memory.content,
            "Prefer small pull requests.\nKeep tests green."
        );
        assert_eq!(memory.scope.kind(), "lab");
        assert_eq!(memory.scope.owner_id(), "a3f91c07");

        assert_eq!(
            Memory::new(MemoryId::generate(), scope(), "   "),
            Err(MemoryError::Empty)
        );
        assert_eq!(
            Memory::new(
                MemoryId::generate(),
                scope(),
                &"a".repeat(MEMORY_MAX_CHARS + 1)
            ),
            Err(MemoryError::TooLong)
        );
        assert_eq!(
            Memory::new(MemoryId::generate(), scope(), "bad\u{7}"),
            Err(MemoryError::ControlCharacters)
        );
    }

    #[test]
    fn every_scope_reports_its_owner() {
        let agent = MemoryScope::Agent(AgentId::parse("1c2d3e4f").unwrap());
        let project = MemoryScope::Project(ProjectId::parse("2d3e4f50").unwrap());
        assert_eq!((agent.kind(), agent.owner_id()), ("agent", "1c2d3e4f"));
        assert_eq!(
            (project.kind(), project.owner_id()),
            ("project", "2d3e4f50")
        );
    }
}
