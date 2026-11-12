//! Project identity and GitHub source (specification §3.2, §8.1–§8.2).

use std::fmt;

use crate::domain::ids::{LabId, ProjectId};
use crate::domain::name::Name;

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

/// A project belongs to exactly one laboratory and may point at one GitHub
/// repository. The GitHub credential is global and never stored here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub lab_id: LabId,
    pub name: Name,
    pub github_source: Option<GithubSource>,
}

impl Project {
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
        assert_eq!(project.name.as_str(), "Customer Portal");
    }
}
