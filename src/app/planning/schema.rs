//! Strict structured planning output, schema version 1 (WP-5-T6).

use serde::Deserialize;

use crate::domain::ids::AgentId;
use crate::domain::skills::{self, SkillTag};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_STORIES: usize = 20;
const MAX_QUESTIONS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Low,
    Medium,
    High,
}

impl Priority {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Complexity {
    Small,
    Medium,
    Large,
}

impl Complexity {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn days(self) -> i64 {
        match self {
            Self::Small => 2,
            Self::Medium => 5,
            Self::Large => 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedStory {
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub priority: Priority,
    pub complexity: Complexity,
    pub required_skills: Vec<SkillTag>,
    pub assigned_agent_id: Option<AgentId>,
    pub assignment_rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPlan {
    pub questions: Vec<String>,
    pub stories: Vec<ValidatedStory>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanError {
    pub field: &'static str,
    pub rule: &'static str,
}

impl PlanError {
    pub fn new(field: &'static str, rule: &'static str) -> Self {
        Self { field, rule }
    }
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the planning output is invalid: field `{}` {}.",
            self.field, self.rule
        )
    }
}

impl std::error::Error for PlanError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlan {
    schema_version: Option<u32>,
    #[serde(default)]
    clarification_questions: Vec<String>,
    #[serde(default)]
    stories: Vec<RawStory>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStory {
    title: Option<String>,
    description: Option<String>,
    #[serde(default)]
    acceptance_criteria: Vec<String>,
    priority: Option<String>,
    complexity: Option<String>,
    #[serde(default)]
    required_skills: Vec<String>,
    #[serde(default)]
    assigned_agent_id: Option<String>,
    #[serde(default)]
    assignment_rationale: Option<String>,
}

pub fn validate(content: &str) -> Result<ValidatedPlan, PlanError> {
    let raw: RawPlan = serde_json::from_str(content).map_err(|error| {
        use serde_json::error::Category;
        match error.classify() {
            Category::Syntax => PlanError::new("output", "is not valid JSON"),
            Category::Eof => PlanError::new("output", "is incomplete JSON"),
            Category::Data => PlanError::new("output", "has an unknown field or wrong type"),
            Category::Io => PlanError::new("output", "could not be read"),
        }
    })?;
    if raw.schema_version != Some(SCHEMA_VERSION) {
        return Err(PlanError::new("schema_version", "must be 1"));
    }
    if raw.stories.len() > MAX_STORIES {
        return Err(PlanError::new("stories", "may contain at most 20 items"));
    }
    if raw.questions_len() > MAX_QUESTIONS {
        return Err(PlanError::new(
            "clarification_questions",
            "may contain at most 10 items",
        ));
    }
    let has_stories = !raw.stories.is_empty();
    let has_questions = !raw.clarification_questions.is_empty();
    if has_stories == has_questions {
        return Err(PlanError::new(
            "stories",
            "and clarification_questions must have exactly one non-empty array",
        ));
    }
    let questions = raw
        .clarification_questions
        .into_iter()
        .map(|question| {
            let question = question.trim().to_owned();
            if question.is_empty() || question.chars().count() > 1_000 || has_controls(&question) {
                Err(PlanError::new(
                    "clarification_questions",
                    "items must be 1 to 1000 characters",
                ))
            } else {
                Ok(question)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let stories = raw
        .stories
        .into_iter()
        .map(validate_story)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ValidatedPlan { questions, stories })
}

impl RawPlan {
    fn questions_len(&self) -> usize {
        self.clarification_questions.len()
    }
}

fn validate_story(raw: RawStory) -> Result<ValidatedStory, PlanError> {
    let title = text(raw.title, "title", 1, 200)?;
    let description = text(raw.description, "description", 1, 4_000)?;
    if raw.acceptance_criteria.is_empty() || raw.acceptance_criteria.len() > 20 {
        return Err(PlanError::new(
            "acceptance_criteria",
            "must contain 1 to 20 items",
        ));
    }
    let acceptance_criteria = raw
        .acceptance_criteria
        .into_iter()
        .map(|item| {
            let item = item.trim().to_owned();
            if item.is_empty() || item.chars().count() > 500 || has_controls(&item) {
                Err(PlanError::new(
                    "acceptance_criteria",
                    "items must be 1 to 500 characters",
                ))
            } else {
                Ok(item)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let priority = raw
        .priority
        .as_deref()
        .and_then(Priority::parse)
        .ok_or_else(|| PlanError::new("priority", "must be low, medium, or high"))?;
    let complexity = raw
        .complexity
        .as_deref()
        .and_then(Complexity::parse)
        .ok_or_else(|| PlanError::new("complexity", "must be small, medium, or large"))?;
    let required_skills = skills::parse_tags(raw.required_skills)
        .map_err(|_| PlanError::new("required_skills", "must be valid skill tags"))?;
    let assigned_agent_id = match raw.assigned_agent_id {
        None => None,
        Some(id) => Some(
            AgentId::parse(&id)
                .ok_or_else(|| PlanError::new("assigned_agent_id", "must be a valid agent ID"))?,
        ),
    };
    let assignment_rationale = raw
        .assignment_rationale
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if assigned_agent_id.is_some() && assignment_rationale.is_none() {
        return Err(PlanError::new(
            "assignment_rationale",
            "is required when an agent is assigned",
        ));
    }
    if assignment_rationale
        .as_ref()
        .is_some_and(|value| value.chars().count() > 2_000)
    {
        return Err(PlanError::new(
            "assignment_rationale",
            "may contain at most 2000 characters",
        ));
    }
    Ok(ValidatedStory {
        title,
        description,
        acceptance_criteria,
        priority,
        complexity,
        required_skills,
        assigned_agent_id,
        assignment_rationale,
    })
}

fn text(
    value: Option<String>,
    field: &'static str,
    min: usize,
    max: usize,
) -> Result<String, PlanError> {
    let value = value.ok_or_else(|| PlanError::new(field, "is required"))?;
    let value = value.trim().to_owned();
    if value.chars().count() < min || value.chars().count() > max || has_controls(&value) {
        return Err(PlanError::new(
            field,
            "has an invalid length or control characters",
        ));
    }
    Ok(value)
}

fn has_controls(value: &str) -> bool {
    value
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> String {
        r#"{
          "schema_version": 1,
          "clarification_questions": [],
          "stories": [{
            "title": "Add reset",
            "description": "Add password reset functionality",
            "acceptance_criteria": ["A reset email is sent"],
            "priority": "high",
            "complexity": "small",
            "required_skills": ["rust", "api"],
            "assigned_agent_id": null,
            "assignment_rationale": null
          }]
        }"#
        .to_owned()
    }

    #[test]
    fn a_valid_plan_round_trips() {
        let plan = validate(&valid()).unwrap();
        assert_eq!(plan.stories.len(), 1);
        assert_eq!(plan.stories[0].priority, Priority::High);
        assert_eq!(plan.stories[0].complexity, Complexity::Small);
        assert_eq!(plan.stories[0].required_skills.len(), 2);
    }

    #[test]
    fn unknown_fields_and_model_dates_are_rejected() {
        let bad = valid().replace(
            "\"title\": \"Add reset\"",
            "\"title\": \"Add reset\", \"due_date\": \"2026-01-01\"",
        );
        let error = validate(&bad).unwrap_err();
        assert_eq!(
            (error.field, error.rule),
            ("output", "has an unknown field or wrong type")
        );
    }

    #[test]
    fn schema_version_is_required_and_exact() {
        let bad = valid().replace("\"schema_version\": 1", "\"schema_version\": 2");
        assert_eq!(
            validate(&bad).unwrap_err(),
            PlanError::new("schema_version", "must be 1")
        );
    }

    #[test]
    fn exactly_one_of_stories_and_questions_must_be_present() {
        let both = valid().replace(
            "\"clarification_questions\": []",
            "\"clarification_questions\": [\"Why?\"]",
        );
        assert!(validate(&both).is_err());
        let neither = r#"{"schema_version":1,"clarification_questions":[],"stories":[]}"#;
        assert!(validate(neither).is_err());
    }

    #[test]
    fn malformed_stories_are_rejected_without_echoing_content() {
        for bad in [
            valid().replace("\"complexity\": \"small\"", "\"complexity\": \"huge\""),
            valid().replace(
                "\"acceptance_criteria\": [\"A reset email is sent\"]",
                "\"acceptance_criteria\": []",
            ),
            valid().replace(
                "\"assigned_agent_id\": null",
                "\"assigned_agent_id\": \"1c2d3e4f\"",
            ),
            valid().replace(
                "\"required_skills\": [\"rust\", \"api\"]",
                "\"required_skills\": [\"bad tag\"]",
            ),
        ] {
            let error = validate(&bad).unwrap_err();
            assert!(!error.to_string().contains("Add reset"));
        }
    }

    #[test]
    fn too_many_stories_and_questions_are_rejected() {
        let mut stories = String::new();
        for index in 0..21 {
            if index > 0 {
                stories.push(',');
            }
            stories.push_str(&format!(
                "{{\"title\":\"T{index}\",\"description\":\"D\",\"acceptance_criteria\":[\"A\"],\"priority\":\"low\",\"complexity\":\"small\",\"required_skills\":[],\"assigned_agent_id\":null,\"assignment_rationale\":null}}"
            ));
        }
        let body = format!(
            "{{\"schema_version\":1,\"clarification_questions\":[],\"stories\":[{stories}]}}"
        );
        assert!(validate(&body).is_err());
        let questions = (0..11)
            .map(|i| format!("\"Q{i}\""))
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(
            "{{\"schema_version\":1,\"clarification_questions\":[{questions}],\"stories\":[]}}"
        );
        assert!(validate(&body).is_err());
    }
}
