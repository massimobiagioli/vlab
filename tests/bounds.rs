//! Resource-bound pairs at the limit and limit+1 (WP6-AC6).

use std::time::Duration;

use vlab::app::planning::schema::validate;
use vlab::domain::budget::{Budget, CancelToken, Deadline, ExecutionLimits};
use vlab::domain::ids::LabId;
use vlab::domain::ids::MemoryId;
use vlab::domain::memory::{MEMORY_MAX_CHARS, Memory, MemoryScope};

fn story(index: usize) -> String {
    format!(
        "{{\"title\":\"T{index}\",\"description\":\"D\",\"acceptance_criteria\":[\"A\"],\"priority\":\"low\",\"complexity\":\"small\",\"required_skills\":[],\"assigned_agent_id\":null,\"assignment_rationale\":null}}"
    )
}

fn plan_with(stories: usize) -> String {
    let items: Vec<String> = (0..stories).map(story).collect();
    format!(
        "{{\"schema_version\":1,\"clarification_questions\":[],\"stories\":[{}]}}",
        items.join(",")
    )
}

#[test]
fn story_count_accepts_twenty_and_rejects_twenty_one() {
    assert!(validate(&plan_with(20)).is_ok());
    assert!(validate(&plan_with(21)).is_err());
}

#[test]
fn memory_content_accepts_two_thousand_and_rejects_two_thousand_one() {
    let scope = MemoryScope::Lab(LabId::parse("a3f91c07").unwrap());
    assert!(
        Memory::new(
            MemoryId::generate(),
            scope.clone(),
            &"a".repeat(MEMORY_MAX_CHARS)
        )
        .is_ok()
    );
    assert!(
        Memory::new(
            MemoryId::generate(),
            scope,
            &"a".repeat(MEMORY_MAX_CHARS + 1)
        )
        .is_err()
    );
}

#[test]
fn production_limits_match_the_documented_values() {
    let limits = ExecutionLimits::default();
    assert_eq!(limits.request_max_bytes, 64 * 1024);
    assert_eq!(limits.response_max_bytes, 256 * 1024);
    assert_eq!(limits.max_stories, 20);
    assert_eq!(limits.max_output_tokens, 4096);
    assert_eq!(limits.total_deadline, Duration::from_secs(240));
    assert_eq!(limits.question_deadline, Duration::from_secs(180));
    assert_eq!(limits.max_attempts, 3);
}

#[test]
fn deadline_and_attempt_boundaries_are_exact() {
    let mut deadline = Deadline::new(Duration::from_millis(1));
    assert!(deadline.consume(Duration::from_millis(1)));
    assert!(deadline.expired());
    assert!(!deadline.consume(Duration::from_millis(1)));

    let mut budget = Budget::new(ExecutionLimits::default(), CancelToken::default());
    assert_eq!(budget.begin_attempt(), Ok(1));
    assert_eq!(budget.begin_attempt(), Ok(2));
    assert_eq!(budget.begin_attempt(), Ok(3));
    assert!(budget.begin_attempt().is_err());
}
