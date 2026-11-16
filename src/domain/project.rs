//! Project identity and source (specification §3.2, §8.1–§8.2).

use std::fmt;
use std::path::Path;

use crate::domain::ids::{LabId, ProjectId};
use crate::domain::name::Name;

/// Longest accepted folder source path, in characters.
pub const FOLDER_PATH_MAX_CHARS: usize = 4_096;

/// A canonical GitHub repository URL (`https://github.com/owner/repo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubSource(String);

impl GithubSource {
    pub fn parse(input: &str) -> Result<Self, GithubSourceError> {
        let trimmed = input.trim().trim_end_matches('/');
        let rest = trimmed
            .strip_prefix("https://github.com/")
            .ok_or(GithubSourceError)?;
        let mut parts = rest.split('/');
        let owner = parts
            .next()
            .filter(|part| valid_segment(part))
            .ok_or(GithubSourceError)?;
        let repo = parts
            .next()
            .filter(|part| valid_segment(part))
            .ok_or(GithubSourceError)?;
        if parts.next().is_some() {
            return Err(GithubSourceError);
        }
        let repo = repo.strip_suffix(".git").unwrap_or(repo);
        if repo.is_empty() {
            return Err(GithubSourceError);
        }
        Ok(Self(format!("https://github.com/{owner}/{repo}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.chars().count() <= 100
        && segment != "."
        && segment != ".."
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

impl fmt::Display for GithubSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a GitHub source must be `https://github.com/<owner>/<repository>`.")]
pub struct GithubSourceError;

/// A local folder source, normalized to an absolute path. Existence and
/// directory checks happen in the workspace adapter, which performs I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSource(String);

impl FolderSource {
    pub fn parse(input: &str) -> Result<Self, FolderSourceError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(FolderSourceError::Empty);
        }
        if trimmed.chars().count() > FOLDER_PATH_MAX_CHARS {
            return Err(FolderSourceError::TooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(FolderSourceError::ControlCharacters);
        }
        let path = Path::new(trimmed);
        if !path.is_absolute() {
            return Err(FolderSourceError::NotAbsolute);
        }
        if path.parent().is_none() {
            return Err(FolderSourceError::Root);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn path(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl fmt::Display for FolderSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FolderSourceError {
    #[error("a folder source is required.")]
    Empty,
    #[error("a folder source may contain at most 4096 characters.")]
    TooLong,
    #[error("a folder source must not contain control characters.")]
    ControlCharacters,
    #[error("a folder source must be an absolute path.")]
    NotAbsolute,
    #[error("a folder source cannot be a filesystem root.")]
    Root,
}

/// The single source of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectSource {
    None,
    Github(GithubSource),
    Folder(FolderSource),
}

impl ProjectSource {
    /// Stable display kind used by lists and events.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Github(_) => "github",
            Self::Folder(_) => "folder",
        }
    }

    /// The URL or path, when a source is present.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::None => None,
            Self::Github(source) => Some(source.as_str()),
            Self::Folder(source) => Some(source.as_str()),
        }
    }
}

/// A project belongs to exactly one laboratory and has at most one source.
/// The GitHub credential is global and never stored here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub lab_id: LabId,
    pub name: Name,
    pub github_source: Option<GithubSource>,
    pub folder_source: Option<FolderSource>,
}

impl Project {
    /// Creates a project with an optional GitHub source. Kept for callers and
    /// tests that predate explicit source types.
    pub fn new(
        id: ProjectId,
        lab_id: LabId,
        name: Name,
        github_source: Option<GithubSource>,
    ) -> Self {
        Self {
            id,
            lab_id,
            name,
            github_source,
            folder_source: None,
        }
    }

    /// Creates a project from an explicit source.
    pub fn with_source(id: ProjectId, lab_id: LabId, name: Name, source: ProjectSource) -> Self {
        let (github_source, folder_source) = match source {
            ProjectSource::None => (None, None),
            ProjectSource::Github(source) => (Some(source), None),
            ProjectSource::Folder(source) => (None, Some(source)),
        };
        Self {
            id,
            lab_id,
            name,
            github_source,
            folder_source,
        }
    }

    pub fn source(&self) -> ProjectSource {
        match (&self.github_source, &self.folder_source) {
            (Some(source), _) => ProjectSource::Github(source.clone()),
            (None, Some(source)) => ProjectSource::Folder(source.clone()),
            _ => ProjectSource::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_sources_are_canonicalized_and_strict() {
        for (raw, expected) in [
            (
                "https://github.com/acme/portal",
                "https://github.com/acme/portal",
            ),
            (
                " https://github.com/acme/portal/ ",
                "https://github.com/acme/portal",
            ),
            (
                "https://github.com/acme/portal.git",
                "https://github.com/acme/portal",
            ),
            (
                "https://github.com/a-b_c.d/repo-1",
                "https://github.com/a-b_c.d/repo-1",
            ),
        ] {
            assert_eq!(
                GithubSource::parse(raw).unwrap().as_str(),
                expected,
                "{raw}"
            );
        }
        for bad in [
            "",
            "http://github.com/acme/portal",
            "https://gitlab.com/acme/portal",
            "https://github.com/acme",
            "https://github.com/acme/portal/extra",
            "https://github.com/../portal",
            "https://github.com/acme/",
            "https://github.com/acme/portal?tab=readme",
        ] {
            assert_eq!(GithubSource::parse(bad), Err(GithubSourceError), "{bad:?}");
        }
    }

    #[test]
    fn a_project_may_have_no_github_source() {
        let project = Project::new(
            ProjectId::parse("0b1c2d3e").unwrap(),
            LabId::parse("a3f91c07").unwrap(),
            Name::parse("Customer Portal").unwrap(),
            None,
        );
        assert_eq!(project.github_source, None);
        assert_eq!(project.folder_source, None);
        assert_eq!(project.source(), ProjectSource::None);
        assert_eq!(project.name.as_str(), "Customer Portal");
    }

    #[test]
    fn folder_sources_are_absolute_trimmed_and_bounded() {
        assert_eq!(
            FolderSource::parse("  /tmp/portal  ").unwrap().as_str(),
            "/tmp/portal"
        );
        for bad in ["", "   ", "relative/path", "/", "bad\u{7}path"] {
            assert!(FolderSource::parse(bad).is_err(), "{bad:?}");
        }
        let too_long = format!("/{}", "a".repeat(FOLDER_PATH_MAX_CHARS));
        assert_eq!(
            FolderSource::parse(&too_long),
            Err(FolderSourceError::TooLong)
        );
    }

    #[test]
    fn a_project_can_have_a_folder_source() {
        let source = FolderSource::parse("/tmp/portal").unwrap();
        let project = Project::with_source(
            ProjectId::parse("0b1c2d3e").unwrap(),
            LabId::parse("a3f91c07").unwrap(),
            Name::parse("Customer Portal").unwrap(),
            ProjectSource::Folder(source.clone()),
        );
        assert_eq!(project.github_source, None);
        assert_eq!(project.folder_source, Some(source));
        assert_eq!(project.source().kind(), "folder");
        assert_eq!(project.source().detail(), Some("/tmp/portal"));
    }
}
