//! Assignment and due-date rules (WP-5-T7).

use time::{Date, Duration, Month};

use crate::app::directory::AgentRecord;
use crate::app::planning::schema::{PlanError, ValidatedPlan, ValidatedStory};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedStory {
    pub story: ValidatedStory,
    pub due_date: String,
    pub assignment_rationale: String,
}

/// Applies membership and date rules. Unknown or out-of-laboratory agents are
/// rejected; the model never supplies a due date.
pub fn apply(
    plan: ValidatedPlan,
    members: &[AgentRecord],
    request_created_at: &str,
) -> Result<Vec<PlannedStory>, PlanError> {
    let request_date = parse_date(request_created_at)
        .ok_or_else(|| PlanError::new("output", "does not have a usable request creation date"))?;
    let member_ids: Vec<&str> = members
        .iter()
        .filter(|member| !member.archived)
        .map(|member| member.id.as_str())
        .collect();
    plan.stories
        .into_iter()
        .map(|story| {
            if let Some(agent) = &story.assigned_agent_id
                && !member_ids.contains(&agent.as_str())
            {
                return Err(PlanError::new(
                    "assigned_agent_id",
                    "must reference an active laboratory member",
                ));
            }
            let due = request_date
                .checked_add(Duration::days(story.complexity.days()))
                .ok_or_else(|| PlanError::new("due_date", "cannot be calculated"))?;
            let rationale = story
                .assignment_rationale
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "No suitable member was available.".to_owned());
            Ok(PlannedStory {
                due_date: format_date(due),
                assignment_rationale: rationale,
                story,
            })
        })
        .collect()
}

fn parse_date(value: &str) -> Option<Date> {
    let date = value.get(..10)?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = Month::try_from(parts.next()?.parse::<u8>().ok()?).ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    Date::from_calendar_date(year, month, day).ok()
}

fn format_date(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month() as u8,
        date.day()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::directory::AgentRecord;
    use crate::app::planning::schema::{Complexity, Priority, ValidatedStory};
    use crate::domain::ids::AgentId;

    fn member(id: &str, archived: bool) -> AgentRecord {
        AgentRecord {
            id: AgentId::parse(id).unwrap(),
            name: "Member".to_owned(),
            role: "dev".to_owned(),
            description: "D".to_owned(),
            skills_json: "[]".to_owned(),
            provider: "deepseek".to_owned(),
            model: "m".to_owned(),
            cv: None,
            archived,
        }
    }

    fn story(assigned: Option<&str>, complexity: Complexity) -> ValidatedStory {
        ValidatedStory {
            title: "T".to_owned(),
            description: "D".to_owned(),
            acceptance_criteria: vec!["A".to_owned()],
            priority: Priority::Medium,
            complexity,
            required_skills: vec![],
            assigned_agent_id: assigned.map(|id| AgentId::parse(id).unwrap()),
            assignment_rationale: Some("Because".to_owned()),
        }
    }

    #[test]
    fn due_dates_follow_complexity_and_leap_years() {
        let cases = [
            (Complexity::Small, "2024-03-01"),
            (Complexity::Medium, "2024-03-04"),
            (Complexity::Large, "2024-03-09"),
        ];
        for (complexity, expected) in cases {
            let plan = ValidatedPlan {
                questions: vec![],
                stories: vec![story(None, complexity)],
            };
            let applied = apply(plan, &[], "2024-02-28T10:00:00Z").unwrap();
            assert_eq!(applied[0].due_date, expected, "{complexity:?}");
        }
    }

    #[test]
    fn month_and_year_boundaries_are_utc_calendar_dates() {
        let plan = ValidatedPlan {
            questions: vec![],
            stories: vec![story(None, Complexity::Medium)],
        };
        assert_eq!(
            apply(plan, &[], "2024-12-30T23:59:59Z").unwrap()[0].due_date,
            "2025-01-04"
        );
    }

    #[test]
    fn assignments_must_reference_an_active_member() {
        let plan = ValidatedPlan {
            questions: vec![],
            stories: vec![story(Some("1c2d3e4f"), Complexity::Small)],
        };
        assert!(
            apply(
                plan.clone(),
                &[member("1c2d3e4f", false)],
                "2024-01-01T00:00:00Z"
            )
            .is_ok()
        );
        assert!(
            apply(
                plan.clone(),
                &[member("1c2d3e4f", true)],
                "2024-01-01T00:00:00Z"
            )
            .is_err()
        );
        assert!(apply(plan, &[member("ffffffff", false)], "2024-01-01T00:00:00Z").is_err());
    }

    #[test]
    fn unassigned_stories_get_an_explanation() {
        let mut s = story(None, Complexity::Small);
        s.assignment_rationale = None;
        let plan = ValidatedPlan {
            questions: vec![],
            stories: vec![s],
        };
        let applied = apply(plan, &[], "2024-01-01T00:00:00Z").unwrap();
        assert_eq!(
            applied[0].assignment_rationale,
            "No suitable member was available."
        );
    }
}
