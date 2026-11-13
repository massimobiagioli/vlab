//! Planning state machine, persistence, and outcomes (WP5-AC7/AC11/AC12).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::kanban::{self, StoryState};
use vlab::app::projects::ProjectStore;
use vlab::app::requests::{self, PlanningOutcome};
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::budget::{CancelToken, ExecutionLimits};
use vlab::domain::clock::TestClock;
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::provider::{ProviderError, ProviderResponse};
use vlab::domain::settings::Settings;
use vlab::domain::skills;
use vlab::infra::db::Database;
use vlab::infra::provider::scripted::ScriptedProvider;
use vlab::obs::RunId;

struct Fixture {
    dir: tempfile::TempDir,
    db: Database,
    project: ProjectId,
}

fn fixture() -> Fixture {
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
    for (id, role, skills_text) in [
        ("1c2d3e4f", Role::Lead, ""),
        ("2d3e4f50", Role::Dev, "rust"),
    ] {
        db.insert_agent(
            &AgentProfile::new(
                AgentId::parse(id).unwrap(),
                Name::parse("Member").unwrap(),
                role,
                Description::parse("Works").unwrap(),
                skills::parse_list(skills_text).unwrap(),
                vlab::domain::settings::ProviderId::DeepSeek,
                vlab::domain::settings::ModelId::parse("deepseek-chat").unwrap(),
                None,
            )
            .unwrap(),
            std::slice::from_ref(&lab),
        )
        .unwrap();
    }
    Fixture { dir, db, project }
}

fn settings() -> Settings {
    Settings::parse("deepseek", "deepseek-chat").unwrap()
}

fn valid_plan() -> String {
    r#"{"schema_version":1,"clarification_questions":[],"stories":[{"title":"Add reset","description":"Reset flow","acceptance_criteria":["Email sent"],"priority":"high","complexity":"small","required_skills":["rust"],"assigned_agent_id":null,"assignment_rationale":"No suitable member."}]}"#.to_owned()
}

fn mixed_plan() -> String {
    r#"{"schema_version":1,"clarification_questions":[],"stories":[
      {"title":"Assigned work","description":"Owned","acceptance_criteria":["Done"],"priority":"high","complexity":"small","required_skills":["rust"],"assigned_agent_id":"2d3e4f50","assignment_rationale":"The developer owns it."},
      {"title":"Waiting work","description":"Unowned","acceptance_criteria":["Done"],"priority":"low","complexity":"small","required_skills":["rust"],"assigned_agent_id":null,"assignment_rationale":"No suitable member."}
    ]}"#
    .to_owned()
}

fn story_states(db: &Database, request_id: &str) -> Vec<(String, String)> {
    db.block_on(async {
        sqlx::query_as("SELECT title, state FROM stories WHERE request_id = ?1 ORDER BY title")
            .bind(request_id)
            .fetch_all(db.pool())
            .await
            .unwrap()
    })
}

fn run(fixture: &Fixture, provider: &ScriptedProvider, text: &str) -> PlanningOutcome {
    requests::plan(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        text,
        &settings(),
        provider,
        ExecutionLimits::default(),
        &TestClock::default(),
        CancelToken::default(),
        RunId::generate(),
    )
    .unwrap()
}

fn status(db: &Database, request_id: &str) -> String {
    db.block_on(async {
        sqlx::query_scalar("SELECT status FROM requests WHERE id = ?1")
            .bind(request_id)
            .fetch_one(db.pool())
            .await
            .unwrap()
    })
}

#[test]
fn a_valid_plan_persists_stories_and_completes_the_request() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![Ok(ProviderResponse {
        content: valid_plan(),
        model: "deepseek-chat".to_owned(),
        usage: None,
        finish_reason: None,
        request_id: None,
    })]);
    let outcome = run(&fixture, &provider, "Add password reset");
    let PlanningOutcome::Planned {
        request_id,
        stories,
    } = outcome
    else {
        panic!("unexpected outcome: {outcome:?}");
    };
    assert_eq!(stories.len(), 1);
    assert_eq!(status(&fixture.db, request_id.as_str()), "PLANNED");
    assert_eq!(
        story_states(&fixture.db, request_id.as_str()),
        vec![("Add reset".to_owned(), "TODO".to_owned())]
    );
    assert_eq!(
        fixture
            .db
            .agent_memberships(&AgentId::parse("1c2d3e4f").unwrap())
            .unwrap()
            .len(),
        1
    );
    let _ = fixture.dir;
}

#[test]
fn assigned_stories_start_in_progress_and_unassigned_stay_todo() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![Ok(ProviderResponse {
        content: mixed_plan(),
        model: "deepseek-chat".to_owned(),
        usage: None,
        finish_reason: None,
        request_id: None,
    })]);
    let outcome = run(&fixture, &provider, "Add two stories");
    let PlanningOutcome::Planned { request_id, .. } = outcome else {
        panic!("unexpected outcome: {outcome:?}");
    };
    assert_eq!(
        story_states(&fixture.db, request_id.as_str()),
        vec![
            ("Assigned work".to_owned(), "IN_PROGRESS".to_owned()),
            ("Waiting work".to_owned(), "TODO".to_owned()),
        ]
    );
    let board = kanban::show(&fixture.db, &fixture.db, fixture.project.as_str()).unwrap();
    assert_eq!(board.counts.in_progress, 1);
    assert_eq!(board.counts.todo, 1);
    assert_eq!(
        board
            .cards_in(StoryState::InProgress)
            .map(|card| card.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Assigned work"]
    );
    assert_eq!(
        board
            .cards_in(StoryState::Todo)
            .map(|card| card.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Waiting work"]
    );
}

struct DbCheckingProvider {
    db: Database,
    calls: std::cell::Cell<usize>,
}

impl vlab::domain::provider::ModelProvider for DbCheckingProvider {
    fn complete(
        &self,
        _: &vlab::domain::provider::ProviderRequest,
    ) -> Result<ProviderResponse, ProviderError> {
        let count: i64 = self.db.block_on(async {
            sqlx::query_scalar("SELECT count(*) FROM requests")
                .fetch_one(self.db.pool())
                .await
                .unwrap()
        });
        assert_eq!(count, 1, "the request must exist before the provider call");
        self.calls.set(self.calls.get() + 1);
        Ok(ProviderResponse {
            content: valid_plan(),
            model: "m".to_owned(),
            usage: None,
            finish_reason: None,
            request_id: None,
        })
    }
}

#[test]
fn the_request_is_persisted_before_the_provider_call() {
    let fixture = fixture();
    let provider = DbCheckingProvider {
        db: fixture.db.clone(),
        calls: std::cell::Cell::new(0),
    };
    let outcome = requests::plan(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        "Add password reset",
        &settings(),
        &provider,
        ExecutionLimits::default(),
        &TestClock::default(),
        CancelToken::default(),
        RunId::generate(),
    )
    .unwrap();
    assert!(matches!(outcome, PlanningOutcome::Planned { .. }));
    assert_eq!(provider.calls.get(), 2, "planning plus memory summary");
}

#[test]
fn a_successful_plan_stores_a_model_summary_memory() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![
        Ok(ProviderResponse {
            content: valid_plan(),
            model: "deepseek-chat".to_owned(),
            usage: None,
            finish_reason: None,
            request_id: None,
        }),
        Ok(ProviderResponse {
            content: "The team will add a password reset flow.".to_owned(),
            model: "deepseek-chat".to_owned(),
            usage: None,
            finish_reason: None,
            request_id: None,
        }),
    ]);
    let outcome = run(&fixture, &provider, "Add password reset");
    assert!(matches!(outcome, PlanningOutcome::Planned { .. }));
    let content: Vec<String> = fixture.db.block_on(async {
        sqlx::query_scalar("SELECT content FROM memories WHERE scope_type = 'project'")
            .fetch_all(fixture.db.pool())
            .await
            .unwrap()
    });
    assert_eq!(content.len(), 1);
    assert_eq!(
        content[0],
        "Planning summary: The team will add a password reset flow."
    );
}

#[test]
fn planning_uses_only_the_assigned_project_roster_when_present() {
    let fixture = fixture();
    let dev = AgentId::parse("2d3e4f50").unwrap();
    fixture
        .db
        .assign_project_agent(&fixture.project, &dev)
        .unwrap();
    let provider = ScriptedProvider::new(vec![Ok(ProviderResponse {
        content: valid_plan(),
        model: "deepseek-chat".to_owned(),
        usage: None,
        finish_reason: None,
        request_id: None,
    })]);
    let outcome = run(&fixture, &provider, "Add password reset");
    assert!(matches!(outcome, PlanningOutcome::Planned { .. }));
    let prompt = &provider.requests()[0].system;
    assert!(prompt.contains("2d3e4f50"), "{prompt}");
    assert!(!prompt.contains("1c2d3e4f"), "{prompt}");
}

#[test]
fn clarification_preserves_questions_and_no_stories() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![Ok(ProviderResponse {
        content: r#"{"schema_version":1,"clarification_questions":["Which users?"],"stories":[]}"#
            .to_owned(),
        model: "m".to_owned(),
        usage: None,
        finish_reason: None,
        request_id: None,
    })]);
    let outcome = run(&fixture, &provider, "Do the thing");
    let PlanningOutcome::NeedsClarification {
        request_id,
        questions,
    } = outcome
    else {
        panic!("unexpected outcome: {outcome:?}");
    };
    assert_eq!(questions, ["Which users?"]);
    assert_eq!(
        status(&fixture.db, request_id.as_str()),
        "NEEDS_CLARIFICATION"
    );
    let count: i64 = fixture.db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM stories")
            .fetch_one(fixture.db.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 0);
}

#[test]
fn invalid_output_fails_without_repair_and_keeps_the_request() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![Ok(ProviderResponse {
        content: r#"{"schema_version":1,"stories":[{"title":"T"}],"clarification_questions":[]}"#
            .to_owned(),
        model: "m".to_owned(),
        usage: None,
        finish_reason: None,
        request_id: None,
    })]);
    let outcome = run(&fixture, &provider, "Do the thing");
    let PlanningOutcome::Failed {
        request_id,
        code,
        validation,
        ..
    } = outcome
    else {
        panic!("unexpected outcome: {outcome:?}");
    };
    assert_eq!(code, "validation");
    assert!(validation);
    assert_eq!(status(&fixture.db, request_id.as_str()), "FAILED");
    assert_eq!(provider.requests().len(), 1, "no automatic repair");
}

#[test]
fn provider_failure_preserves_the_request_as_failed() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![Err(ProviderError::Authentication)]);
    let outcome = run(&fixture, &provider, "Do the thing");
    let PlanningOutcome::Failed {
        request_id,
        code,
        validation,
        ..
    } = outcome
    else {
        panic!("unexpected outcome: {outcome:?}");
    };
    assert_eq!(code, "provider_authentication");
    assert!(!validation);
    assert_eq!(status(&fixture.db, request_id.as_str()), "FAILED");
}

#[test]
fn stale_planning_is_recovered_without_a_provider_call() {
    let fixture = fixture();
    fixture
        .db
        .execute(
            "INSERT INTO requests (id, project_id, content, status, planning_started_at, created_at, updated_at) \
             VALUES ('3e4f5061', '0b1c2d3e', 'R', 'PLANNING', '2000-01-01T00:00:00Z', '2000-01-01T00:00:00Z', '2000-01-01T00:00:00Z')",
        )
        .unwrap();
    let recovered = requests::recover_stale(&fixture.db).unwrap();
    assert_eq!(recovered, 1);
    assert_eq!(status(&fixture.db, "3e4f5061"), "FAILED");
}
