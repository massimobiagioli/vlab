//! Simulated work/review workflow (ADR-0048).

mod support;

use vlab::app::context::{save_active_lab, save_active_project};
use vlab::app::directory::Directory;
use vlab::app::kanban;
use vlab::app::projects::ProjectStore;
use vlab::app::workflow;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::budget::{CancelToken, ExecutionLimits};
use vlab::domain::clock::TestClock;
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::provider::{ProviderError, ProviderResponse};
use vlab::domain::settings::{ModelId, ProviderId, Settings};
use vlab::domain::skills;
use vlab::infra::db::Database;
use vlab::infra::provider::scripted::ScriptedProvider;
use vlab::obs::RunId;

struct Fixture {
    dir: tempfile::TempDir,
    db: Database,
    project: ProjectId,
}

fn fixture(with_qa: bool) -> Fixture {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(RunId::generate()).unwrap();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
    let project = ProjectId::parse("0b1c2d3e").unwrap();
    db.insert_project(&Project::new(
        project.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    save_active_project(&db, &project).unwrap();
    for (id, name, role, skills_text) in [
        ("1c2d3e4f", "Grace", Role::Lead, ""),
        ("2d3e4f50", "Ada", Role::Dev, "rust"),
    ] {
        db.insert_agent(
            &AgentProfile::new(
                AgentId::parse(id).unwrap(),
                Name::parse(name).unwrap(),
                role,
                Description::parse("Works").unwrap(),
                skills::parse_list(skills_text).unwrap(),
                ProviderId::DeepSeek,
                ModelId::parse("deepseek-chat").unwrap(),
                None,
            )
            .unwrap(),
            std::slice::from_ref(&lab),
        )
        .unwrap();
    }
    if with_qa {
        db.insert_agent(
            &AgentProfile::new(
                AgentId::parse("3e4f5061").unwrap(),
                Name::parse("Quinn").unwrap(),
                Role::Qa,
                Description::parse("Reviews").unwrap(),
                skills::parse_list("testing").unwrap(),
                ProviderId::DeepSeek,
                ModelId::parse("deepseek-chat").unwrap(),
                None,
            )
            .unwrap(),
            std::slice::from_ref(&lab),
        )
        .unwrap();
    }
    db.execute(
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
         VALUES ('4f506172', '0b1c2d3e', 'Add reset', 'PLANNED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, assigned_agent_id, assignment_rationale, state, due_date, created_at, updated_at) \
         VALUES ('5a617283', '0b1c2d3e', '4f506172', 'Add reset', 'Reset flow', '[\"Email sent\"]', 'high', 'small', '[\"rust\"]', '2d3e4f50', 'Rust developer.', 'IN_PROGRESS', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    Fixture { dir, db, project }
}

fn settings() -> Settings {
    Settings::parse("deepseek", "deepseek-chat").unwrap()
}

fn response(content: &str) -> ProviderResponse {
    ProviderResponse {
        content: content.to_owned(),
        model: "deepseek-chat".to_owned(),
        usage: None,
        finish_reason: None,
        request_id: None,
    }
}

fn run(fixture: &Fixture, provider: &ScriptedProvider) -> workflow::WorkflowReport {
    workflow::run(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &settings(),
        provider,
        ExecutionLimits::default(),
        &TestClock::default(),
        CancelToken::default(),
        RunId::generate(),
    )
    .unwrap()
}

#[test]
fn qa_reviews_and_the_story_is_done_with_an_outcome() {
    let fixture = fixture(true);
    let provider = ScriptedProvider::new(vec![
        Ok(response(
            r#"{"work_outcome":"Implemented the reset flow."}"#,
        )),
        Ok(response(r#"{"review_outcome":"Looks good."}"#)),
    ]);
    let report = run(&fixture, &provider);
    assert_eq!(
        report,
        workflow::WorkflowReport {
            completed: 1,
            failed: 0,
            remaining: 0
        }
    );
    let requests = provider.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1].system.contains("Quinn"),
        "{}",
        requests[1].system
    );
    let story = kanban::story(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &vlab::domain::ids::StoryId::parse("5a617283").unwrap(),
    )
    .unwrap();
    assert_eq!(story.state, kanban::StoryState::Done);
    let outcome = story.outcome.unwrap();
    assert_eq!(outcome.work, "Implemented the reset flow.");
    assert_eq!(outcome.review, "Looks good.");
    assert_eq!(outcome.reviewer, "Quinn");
    assert!(!outcome.completed_at.is_empty());
    let _ = fixture.dir;
}

#[test]
fn the_tech_lead_reviews_when_no_qa_exists() {
    let fixture = fixture(false);
    let provider = ScriptedProvider::new(vec![
        Ok(response(r#"{"work_outcome":"Work."}"#)),
        Ok(response(r#"{"review_outcome":"Review."}"#)),
    ]);
    run(&fixture, &provider);
    let requests = provider.requests();
    assert!(
        requests[1].system.contains("Grace"),
        "{}",
        requests[1].system
    );
    let story = kanban::story(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &vlab::domain::ids::StoryId::parse("5a617283").unwrap(),
    )
    .unwrap();
    assert_eq!(story.outcome.unwrap().reviewer, "Grace");
}

#[test]
fn invalid_work_output_leaves_the_story_in_progress() {
    let fixture = fixture(true);
    let provider = ScriptedProvider::new(vec![Ok(response(r#"{"bad":true}"#))]);
    let report = run(&fixture, &provider);
    assert_eq!(report.failed, 1);
    assert_eq!(report.completed, 0);
    let story = kanban::story(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &vlab::domain::ids::StoryId::parse("5a617283").unwrap(),
    )
    .unwrap();
    assert_eq!(story.state, kanban::StoryState::InProgress);
    assert!(story.outcome.is_none());
}

#[test]
fn provider_failure_does_not_change_the_state() {
    let fixture = fixture(true);
    let provider = ScriptedProvider::new(vec![Err(ProviderError::Timeout)]);
    let report = run(&fixture, &provider);
    assert_eq!(report.failed, 1);
    let story = kanban::story(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &vlab::domain::ids::StoryId::parse("5a617283").unwrap(),
    )
    .unwrap();
    assert_eq!(story.state, kanban::StoryState::InProgress);
}

#[test]
fn unassigned_and_completed_stories_are_not_processed() {
    let fixture = fixture(true);
    fixture
        .db
        .execute(
            "UPDATE stories SET state = 'TODO', assigned_agent_id = NULL WHERE id = '5a617283'",
        )
        .unwrap();
    let provider = ScriptedProvider::new(Vec::new());
    let report = run(&fixture, &provider);
    assert_eq!(report, workflow::WorkflowReport::default());
    assert!(provider.requests().is_empty());
}
