//! Recurring project schedules (ADR-0057).

mod support;

use support::Sandbox;
use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::execution::ActionRunner;
use vlab::app::projects::ProjectStore;
use vlab::app::schedules::{self, ScheduleStore};
use vlab::app::worktrees::{WorkflowEnvironment, Worktree, WorktreeError};
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::budget::{CancelToken, ExecutionLimits};
use vlab::domain::clock::TestClock;
use vlab::domain::ids::{AgentId, LabId, ProjectId, ScheduleId, StoryId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::provider::ProviderResponse;
use vlab::domain::schedule::ScheduleInterval;
use vlab::domain::settings::{ModelId, ProviderId, Settings};
use vlab::domain::skills;
use vlab::infra::db::Database;
use vlab::infra::provider::scripted::ScriptedProvider;
use vlab::obs::RunId;

struct Fixture {
    _sandbox: Sandbox,
    db: Database,
    project: ProjectId,
    lead: AgentId,
    dev: AgentId,
}

fn fixture() -> Fixture {
    let sandbox = Sandbox::new();
    let db = sandbox.seed_database();
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
    let lead = AgentId::parse("10000001").unwrap();
    let dev = AgentId::parse("10000002").unwrap();
    for (id, role, skills_text) in [(&lead, Role::Lead, ""), (&dev, Role::Dev, "rust")] {
        db.insert_agent(
            &AgentProfile::new(
                id.clone(),
                Name::parse("Member").unwrap(),
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
    Fixture {
        _sandbox: sandbox,
        db,
        project,
        lead,
        dev,
    }
}

fn settings() -> Settings {
    Settings::parse("deepseek", "deepseek-chat").unwrap()
}

fn unassigned_plan() -> String {
    r#"{"schema_version":1,"clarification_questions":[],"stories":[{"title":"Maintenance","description":"Check dependencies","acceptance_criteria":["Checked"],"priority":"medium","complexity":"small","required_skills":["rust"],"assigned_agent_id":null,"assignment_rationale":"No suitable member."}]}"#.to_owned()
}

struct NoEnvironment;

impl WorkflowEnvironment for NoEnvironment {
    fn prepare(&self, _: &ProjectId, _: &StoryId) -> Result<Worktree, WorktreeError> {
        panic!("the workflow should not prepare a worktree for an unassigned story")
    }

    fn actions(&self, _: &Worktree) -> Box<dyn ActionRunner + '_> {
        panic!("the workflow should not run actions for an unassigned story")
    }
}

fn make_due(db: &Database, id: &ScheduleId) {
    db.block_on(async {
        sqlx::query("UPDATE schedules SET next_run_at = '2000-01-01T00:00:00Z' WHERE id = ?1")
            .bind(id.as_str())
            .execute(db.pool())
            .await
            .unwrap();
    });
}

#[test]
fn a_due_schedule_creates_a_planned_request_and_advances() {
    let fixture = fixture();
    let schedule = schedules::create(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &fixture.dev,
        "Weekly maintenance",
        "Check dependencies and review security.",
        ScheduleInterval::Weekly,
    )
    .unwrap();
    make_due(&fixture.db, &schedule.id);
    let provider = ScriptedProvider::new(vec![Ok(ProviderResponse {
        content: unassigned_plan(),
        model: "deepseek-chat".to_owned(),
        usage: None,
        finish_reason: None,
        request_id: None,
    })]);
    let report = schedules::run_due(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &settings(),
        &provider,
        &NoEnvironment,
        ExecutionLimits::default(),
        &TestClock::default(),
        CancelToken::default(),
        RunId::generate(),
    )
    .unwrap();
    assert_eq!(report.ran, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(report.remaining, 0);

    let stored = fixture.db.schedule(&schedule.id).unwrap().unwrap();
    assert!(stored.last_run_at.is_some());
    assert!(stored.last_request_id.is_some());
    assert_eq!(stored.last_request_status.as_deref(), Some("PLANNED"));
    assert_ne!(stored.next_run_at, "2000-01-01T00:00:00Z");
}

#[test]
fn an_inactive_owner_records_an_error_without_a_request() {
    let fixture = fixture();
    let schedule = schedules::create(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &fixture.dev,
        "Weekly maintenance",
        "Check dependencies.",
        ScheduleInterval::Weekly,
    )
    .unwrap();
    make_due(&fixture.db, &schedule.id);
    fixture
        .db
        .execute("UPDATE agents SET archived_at = '2026-01-01T00:00:00Z' WHERE id = '10000002'")
        .unwrap();
    let provider = ScriptedProvider::new(Vec::new());
    let report = schedules::run_due(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &settings(),
        &provider,
        &NoEnvironment,
        ExecutionLimits::default(),
        &TestClock::default(),
        CancelToken::default(),
        RunId::generate(),
    )
    .unwrap();
    assert_eq!(report.ran, 0);
    assert_eq!(report.failed, 1);
    let stored = fixture.db.schedule(&schedule.id).unwrap().unwrap();
    assert_eq!(stored.last_error_code.as_deref(), Some("agent_inactive"));
    assert!(stored.last_request_id.is_none());
}

#[test]
fn only_active_members_can_own_a_schedule() {
    let fixture = fixture();
    let outsider = AgentId::parse("ffffffff").unwrap();
    let error = schedules::create(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &outsider,
        "Weekly maintenance",
        "Check dependencies.",
        ScheduleInterval::Weekly,
    )
    .unwrap_err();
    assert!(matches!(error, vlab::app::AppError::Precondition { .. }));
    assert!(
        fixture
            .db
            .project_schedules(&fixture.project, &LabId::parse("a3f91c07").unwrap())
            .unwrap()
            .unwrap()
            .schedules
            .is_empty()
    );
}

#[test]
fn a_schedule_can_be_paused_resumed_and_deleted() {
    let fixture = fixture();
    let schedule = schedules::create(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &fixture.lead,
        "Weekly maintenance",
        "Check dependencies.",
        ScheduleInterval::Weekly,
    )
    .unwrap();
    let paused =
        schedules::set_enabled(&fixture.db, &fixture.db, schedule.id.as_str(), false).unwrap();
    assert!(!paused.enabled);
    let resumed =
        schedules::set_enabled(&fixture.db, &fixture.db, schedule.id.as_str(), true).unwrap();
    assert!(resumed.enabled);
    schedules::delete(&fixture.db, &fixture.db, schedule.id.as_str()).unwrap();
    assert!(fixture.db.schedule(&schedule.id).unwrap().is_none());
}
