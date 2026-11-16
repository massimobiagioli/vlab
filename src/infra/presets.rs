//! The tracked preset agent catalog (ADR-0052).
//!
//! Parsing is strict and bounded. Error reasons are built from typed parser
//! information and field names only, so no catalog text can reach the user
//! through a parse failure.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use serde::Deserialize;
use serde_saphyr::Error as YamlError;

use crate::app::AppError;
use crate::domain::preset::{PRESET_MAX_AGENTS, PresetAgent, PresetRoster};

/// Largest accepted preset file.
pub const MAX_PRESET_BYTES: usize = 512 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDocument {
    version: u32,
    agents: Vec<RawAgent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgent {
    name: String,
    role: String,
    description: String,
    skills: Vec<String>,
    cv: Option<String>,
}

/// Parses and validates a preset document. Pure apart from its input.
pub fn parse_presets(bytes: &[u8]) -> Result<PresetRoster, String> {
    if bytes.len() > MAX_PRESET_BYTES {
        return Err(format!("it is larger than {} KiB", MAX_PRESET_BYTES / 1024));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "it is not valid UTF-8".to_owned())?;
    let raw: RawDocument = serde_saphyr::from_str(text).map_err(|error| yaml_reason(&error))?;
    if raw.version != 1 {
        return Err("unsupported `version`; only 1 is accepted".to_owned());
    }
    if raw.agents.len() > PRESET_MAX_AGENTS {
        return Err(format!(
            "a preset roster may contain at most {PRESET_MAX_AGENTS} agents"
        ));
    }
    let mut agents = Vec::with_capacity(raw.agents.len());
    for (index, raw_agent) in raw.agents.iter().enumerate() {
        let agent = PresetAgent::parse(
            &raw_agent.name,
            &raw_agent.role,
            &raw_agent.description,
            &raw_agent.skills,
            raw_agent.cv.clone(),
        )
        .map_err(|error| format!("`agents[{index}]`: {error}"))?;
        agents.push(agent);
    }
    PresetRoster::new(agents).map_err(|error| error.to_string())
}

/// Reads and validates the preset file, mapping failures to the CLI contract.
/// Invalid input is reported before the caller opens the database.
pub fn load_presets(path: &Path) -> Result<PresetRoster, AppError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(AppError::Precondition {
                problem: format!("the preset file `{}` does not exist.", path.display()),
                hint: "Pass the path to a YAML preset file, for example `assets/agents.yaml`."
                    .to_owned(),
            });
        }
        Err(error) => {
            return Err(AppError::Io {
                context: "reading the preset file",
                kind: error.kind(),
            });
        }
    };
    let mut bytes = Vec::new();
    // One byte over the limit is enough to detect an oversized file.
    file.take(MAX_PRESET_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::Io {
            context: "reading the preset file",
            kind: error.kind(),
        })?;
    parse_presets(&bytes).map_err(|reason| AppError::Validation {
        problem: format!("the preset file is invalid: {reason}."),
        hint: "Fix the preset file and run the command again.".to_owned(),
    })
}

/// A content-free description of a parse failure.
fn yaml_reason(error: &YamlError) -> String {
    let error = error.without_snippet();
    let line = error
        .location()
        .map(|location| location.line())
        .filter(|line| *line > 0);
    let at = |line: Option<u64>| {
        line.map(|line| format!(" (line {line})"))
            .unwrap_or_default()
    };
    match error {
        YamlError::SerdeUnknownField { .. } => format!(
            "it contains an unknown field{}; only `version`, `agents`, and the agent fields `name`, `role`, `description`, `skills`, and `cv` are allowed",
            at(line)
        ),
        YamlError::SerdeMissingField { field, .. } => {
            format!("the required field `{field}` is missing")
        }
        YamlError::SerdeInvalidType { .. } | YamlError::SerdeInvalidValue { .. } => {
            format!("it has a field with an invalid type{}", at(line))
        }
        YamlError::MultipleDocuments { .. } => "it contains more than one YAML document".to_owned(),
        YamlError::Eof { .. } => "it is empty or incomplete".to_owned(),
        YamlError::Budget { .. } => "it exceeds the YAML complexity limits".to_owned(),
        _ => format!("it is not valid YAML{}", at(line)),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::agent::Role;

    fn tracked_catalog() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/agents.yaml")
    }

    const SMALL: &str = "\
version: 1
agents:
  - name: Klaus Vandenberg
    role: lead
    description: Coordinates planning.
    skills: [architecture, planning]
    cv: |
      Fictional Tech Lead.
  - name: Bram Kellerhof
    role: dev
    description: Writes backend services.
    skills: [rust, sqlite]
";

    #[test]
    fn a_valid_document_parses_and_trims_an_optional_cv() {
        let roster = parse_presets(SMALL.as_bytes()).unwrap();
        assert_eq!(roster.agents().len(), 2);
        assert_eq!(roster.agents()[0].role, Role::Lead);
        assert!(
            roster.agents()[0]
                .cv
                .as_deref()
                .unwrap()
                .contains("Fictional")
        );
    }

    #[test]
    fn the_tracked_catalog_is_valid_and_has_thirty_mixed_agents() {
        let bytes = std::fs::read(tracked_catalog()).unwrap();
        let roster = parse_presets(&bytes).unwrap();
        assert_eq!(roster.agents().len(), 30);
        let leads = roster
            .agents()
            .iter()
            .filter(|agent| agent.role == Role::Lead)
            .count();
        assert_eq!(leads, 1);
        for role in [Role::Dev, Role::Analyst, Role::Qa] {
            assert!(
                roster.agents().iter().any(|agent| agent.role == role),
                "missing role {}",
                role.as_str()
            );
        }
        assert!(
            roster
                .agents()
                .iter()
                .filter(|agent| agent.role != Role::Lead)
                .all(|agent| !agent.skills.is_empty())
        );
    }

    #[test]
    fn invalid_documents_have_specific_content_free_reasons() {
        let cases: &[(&str, &str)] = &[
            (
                "version: 1\nagents:\n  - name: A\n    role: lead\n    description: D\n    skills: []\n    extra: x\n",
                "unknown field",
            ),
            (
                "version: 1\nagents:\n  - name: A\n    role: lead\n    skills: []\n",
                "required field `description` is missing",
            ),
            (
                "version: 2\nagents:\n  - name: A\n    role: lead\n    description: D\n    skills: []\n",
                "only 1 is accepted",
            ),
            (
                "version: 1\nagents:\n  - name: A\n    role: boss\n    description: D\n    skills: [rust]\n",
                "role is invalid",
            ),
            (
                "version: 1\nagents:\n  - name: A\n    role: lead\n    description: D\n    skills: []\n  - name: B\n    role: lead\n    description: D\n    skills: []\n",
                "exactly one `lead`",
            ),
            (
                "version: 1\nagents:\n  - name: A\n    role: lead\n    description: D\n    skills: []\n  - name: a\n    role: dev\n    description: D\n    skills: [rust]\n",
                "appears more than once",
            ),
            (
                "version: 1\nagents:\n  - name: A\n    role: lead\n    description: D\n    skills: []\n  - name: B\n    role: dev\n    description: D\n    skills: []\n",
                "needs at least one skill",
            ),
        ];
        for (yaml, expected) in cases {
            let reason = parse_presets(yaml.as_bytes()).unwrap_err();
            assert!(reason.contains(expected), "{expected:?}: {reason}");
        }
    }

    #[test]
    fn size_encoding_and_yaml_errors_are_reported() {
        let big = format!("version: 1\nagents: []\n#{}", "x".repeat(MAX_PRESET_BYTES));
        assert_eq!(
            parse_presets(big.as_bytes()).unwrap_err(),
            "it is larger than 512 KiB"
        );
        assert_eq!(
            parse_presets(b"version: \xff\n").unwrap_err(),
            "it is not valid UTF-8"
        );
        assert!(
            parse_presets(b"version: [\n")
                .unwrap_err()
                .contains("not valid YAML")
        );
        assert!(
            parse_presets(b"")
                .unwrap_err()
                .contains("empty or incomplete")
        );
    }

    #[test]
    fn a_missing_file_is_a_precondition_and_creates_nothing() {
        let error = load_presets(Path::new("/nonexistent/vlab/agents.yaml")).unwrap_err();
        assert!(matches!(error, AppError::Precondition { .. }), "{error:?}");
    }
}
