//! Markdown agent definitions (ADR-0060).
//!
//! A definition is UTF-8 markdown with strict YAML frontmatter. The body is the
//! optional CV. Parsing errors are content-free.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use serde::Deserialize;

use crate::app::AppError;
use crate::domain::preset::PresetAgent;

/// Largest accepted definition file, including the CV body.
pub const MAX_AGENT_DEFINITION_BYTES: usize = 128 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDefinition {
    name: String,
    role: String,
    description: String,
    skills: Vec<String>,
}

/// Parses a definition file into the same validated shape as a preset agent.
pub fn parse_agent_definition(bytes: &[u8]) -> Result<PresetAgent, String> {
    if bytes.len() > MAX_AGENT_DEFINITION_BYTES {
        return Err(format!(
            "it is larger than {} KiB",
            MAX_AGENT_DEFINITION_BYTES / 1024
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "it is not valid UTF-8".to_owned())?;
    let (frontmatter, body) = split_frontmatter(text)?;
    let raw: RawDefinition = serde_saphyr::from_str(frontmatter)
        .map_err(|_| "the frontmatter is not valid strict YAML".to_owned())?;
    let cv = if body.trim().is_empty() {
        None
    } else {
        Some(body.trim().to_owned())
    };
    PresetAgent::parse(&raw.name, &raw.role, &raw.description, &raw.skills, cv)
        .map_err(|error| error.to_string())
}

/// Reads and validates a `.md` definition, mapping failures to CLI errors.
pub fn load_agent_definition(path: &Path) -> Result<PresetAgent, AppError> {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
    {
        return Err(AppError::Usage {
            message: "an agent definition must be a `.md` file.".to_owned(),
        });
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => AppError::Precondition {
            problem: format!("the agent definition `{}` does not exist.", path.display()),
            hint: "Pass an existing markdown file with `SRC=<file.md>`.".to_owned(),
        },
        kind => AppError::Io {
            context: "reading the agent definition",
            kind,
        },
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::Precondition {
            problem: "the agent definition is not a regular file.".to_owned(),
            hint: "Use a regular `.md` file, not a symlink or directory.".to_owned(),
        });
    }
    if metadata.len() > MAX_AGENT_DEFINITION_BYTES as u64 {
        return Err(AppError::Validation {
            problem: format!(
                "the agent definition is larger than {} KiB.",
                MAX_AGENT_DEFINITION_BYTES / 1024
            ),
            hint: "Shorten the CV or frontmatter and retry.".to_owned(),
        });
    }
    let file = File::open(path).map_err(|error| AppError::Io {
        context: "reading the agent definition",
        kind: error.kind(),
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_AGENT_DEFINITION_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::Io {
            context: "reading the agent definition",
            kind: error.kind(),
        })?;
    parse_agent_definition(&bytes).map_err(|reason| AppError::Validation {
        problem: format!("the agent definition is invalid: {reason}."),
        hint: "Fix the markdown frontmatter or CV and retry.".to_owned(),
    })
}

fn split_frontmatter(text: &str) -> Result<(&str, &str), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    let first = lines.next().ok_or_else(|| "it is empty".to_owned())?;
    if first.trim_end_matches(['\r', '\n']).trim() != "---" {
        return Err("it must start with a `---` frontmatter delimiter".to_owned());
    }
    let front_start = first.len();
    let mut cursor = front_start;
    for line in lines {
        let line_without_newline = line.trim_end_matches(['\r', '\n']);
        if line_without_newline.trim() == "---" {
            let front = &text[front_start..cursor];
            let body = &text[cursor + line.len()..];
            return Ok((front, body));
        }
        cursor += line.len();
    }
    Err("the frontmatter is missing its closing `---`".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent::Role;

    #[test]
    fn a_definition_uses_frontmatter_and_body_as_cv() {
        let input = "\u{feff}---\nname: Ada\nrole: dev\ndescription: Backend developer\nskills:\n  - rust\n  - sqlite\n---\n# Profile\n\nKeep tests green.\n";
        let agent = parse_agent_definition(input.as_bytes()).unwrap();
        assert_eq!(agent.name.as_str(), "Ada");
        assert_eq!(agent.role, Role::Dev);
        assert_eq!(agent.skills.len(), 2);
        assert!(agent.cv.as_deref().unwrap().contains("Keep tests green."));
    }

    #[test]
    fn malformed_definitions_are_rejected_without_echoing_content() {
        for (input, expected) in [
            ("name: Ada\n", "must start"),
            ("---\nname: Ada\nrole: dev\n", "closing"),
            (
                "---\nname: Ada\nrole: dev\ndescription: D\nskills: [rust]\nextra: x\n---\n",
                "not valid strict YAML",
            ),
            (
                "---\nname: Ada\nrole: boss\ndescription: D\nskills: [rust]\n---\n",
                "role is invalid",
            ),
            (
                "---\nname: Ada\nrole: dev\ndescription: D\nskills: [two words]\n---\n",
                "skills are invalid",
            ),
        ] {
            let reason = parse_agent_definition(input.as_bytes()).unwrap_err();
            assert!(reason.contains(expected), "{expected:?}: {reason}");
        }
        assert_eq!(
            parse_agent_definition(b"\xff").unwrap_err(),
            "it is not valid UTF-8"
        );
    }
}
