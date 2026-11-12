//! Skill tags (A-06).
//!
//! A tag is a lowercase slug of 1–32 characters, `[a-z0-9-]`. Input is
//! trimmed and lowercased, empty entries are ignored, and duplicates are
//! removed while preserving first-seen order. An agent carries at most 20
//! tags.
//!
//! Storage uses a JSON array of strings (`skills_json`). Because tags cannot
//! contain quotes, backslashes, or control characters, encoding needs no
//! escaping and decoding can reject anything outside that small grammar.

use std::fmt;

/// Longest accepted tag, in characters.
pub const SKILL_MAX_CHARS: usize = 32;

/// Maximum tags per agent.
pub const MAX_SKILLS: usize = 20;

/// One normalized skill tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SkillTag(String);

impl SkillTag {
    pub fn parse(input: &str) -> Result<Self, SkillError> {
        let normalized = input.trim().to_ascii_lowercase();
        let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        if normalized.is_empty()
            || normalized.chars().count() > SKILL_MAX_CHARS
            || !normalized.chars().all(allowed)
        {
            return Err(SkillError::InvalidTag);
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SkillTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SkillError {
    #[error("a skill must be 1 to 32 letters, digits, or `-`.")]
    InvalidTag,
    #[error("an agent may have at most 20 skills.")]
    TooMany,
    #[error("the stored skill list is malformed.")]
    MalformedStorage,
}

/// Normalizes a collection of tags: trim, lowercase, drop empties, deduplicate.
pub fn parse_tags<I, S>(inputs: I) -> Result<Vec<SkillTag>, SkillError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut tags = Vec::new();
    for input in inputs {
        let trimmed = input.as_ref().trim();
        if trimmed.is_empty() {
            continue;
        }
        let tag = SkillTag::parse(trimmed)?;
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    if tags.len() > MAX_SKILLS {
        return Err(SkillError::TooMany);
    }
    Ok(tags)
}

/// Parses a comma-separated list, as typed in the wizard. Spaces inside an
/// entry are not separators, so `not a skill` is rejected rather than silently
/// becoming three tags.
pub fn parse_list(input: &str) -> Result<Vec<SkillTag>, SkillError> {
    parse_tags(input.split(','))
}

/// Serializes tags to the JSON array stored in `agents.skills_json`.
pub fn encode(tags: &[SkillTag]) -> String {
    let mut out = String::from("[");
    for (index, tag) in tags.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(tag.as_str());
        out.push('"');
    }
    out.push(']');
    out
}

/// Parses the stored JSON array, rejecting anything outside the tag grammar.
pub fn decode(json: &str) -> Result<Vec<SkillTag>, SkillError> {
    let trimmed = json.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or(SkillError::MalformedStorage)?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut raw = Vec::new();
    for element in inner.split(',') {
        let element = element.trim();
        let quoted = element
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .ok_or(SkillError::MalformedStorage)?;
        if quoted.contains('"') || quoted.contains('\\') {
            return Err(SkillError::MalformedStorage);
        }
        raw.push(quoted.to_owned());
    }
    parse_tags(raw).map_err(|error| match error {
        SkillError::TooMany => SkillError::TooMany,
        _ => SkillError::MalformedStorage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(values: &[&str]) -> Vec<SkillTag> {
        values
            .iter()
            .map(|value| SkillTag::parse(value).unwrap())
            .collect()
    }

    #[test]
    fn tags_are_lowercased_trimmed_and_deduplicated() {
        let parsed = parse_list(" Rust, rust , API ,,  ").unwrap();
        assert_eq!(parsed, tags(&["rust", "api"]));
        assert_eq!(
            parse_tags(["Rust", "RUST", "api"]).unwrap(),
            tags(&["rust", "api"])
        );
    }

    #[test]
    fn invalid_tags_are_rejected() {
        for bad in ["", "   ", "two words", "a/b", "é", "a.b", "under_score"] {
            assert_eq!(SkillTag::parse(bad), Err(SkillError::InvalidTag), "{bad:?}");
        }
        assert_eq!(
            SkillTag::parse(&"a".repeat(SKILL_MAX_CHARS + 1)),
            Err(SkillError::InvalidTag)
        );
        assert!(
            parse_list(
                &(0..MAX_SKILLS + 1)
                    .map(|i| format!("s{i}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
            .is_err()
        );
    }

    #[test]
    fn the_storage_encoding_round_trips_without_escaping() {
        let original = tags(&["rust", "api", "qa"]);
        let encoded = encode(&original);
        assert_eq!(encoded, r#"["rust","api","qa"]"#);
        assert_eq!(decode(&encoded).unwrap(), original);
        assert_eq!(encode(&[]), "[]");
        assert_eq!(decode("[]").unwrap(), Vec::new());
    }

    #[test]
    fn malformed_storage_is_rejected() {
        for bad in [
            "",
            "rust",
            "[\"rust\"",
            "[\"rust\\\"]",
            "[rust]",
            "[\"rust\"]x",
        ] {
            assert_eq!(decode(bad), Err(SkillError::MalformedStorage), "{bad:?}");
        }
    }
}
