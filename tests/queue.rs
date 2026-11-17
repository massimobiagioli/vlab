//! Background request queue (ADR-0061).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::app::queue;
use vlab::app::requests::{self, PlanningOutcome};
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::budget::{CancelToken, ExecutionLimits};
use vlab::domain::clock::TestClock;
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::provider::ProviderResponse;
use vlab::domain::settings::Settings;
use vlab::domain::skills;
use vlab::infra::db::Database;
use vlab::infra::provider::scripted::ScriptedProvider;
use vlab::obs::RunId;

fn fixture() -> (tempfile::TempDir, Database, LabId, ProjectId) {
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
    (dir, db, lab, project)
}

fn settings() -> Settings {
    Settings::parse("deepseek", "deepseek-chat").unwrap()
}

fn plan_response() -> Result<ProviderResponse, vlab::domain::provider::ProviderError> {
    Ok(ProviderResponse {
        content: r#"{"schema_version":1,"clarification_questions":[],"stories":[{"title":"Add reset","description":"Reset flow","acceptance_criteria":["Email sent"],"priority":"high","complexity":"small","required_skills":["rust"],"assigned_agent_id":null,"assignment_rationale":"No suitable member."}]}"#.to_owned(),
        model: "deepseek-chat".to_owned(),
        usage: None,
        finish_reason: None,
        request_id: None,
    })
}

fn status(db: &Database, id: &str) -> String {
    db.block_on(async {
        sqlx::query_scalar("SELECT status FROM requests WHERE id = ?1")
            .bind(id)
            .fetch_one(db.pool())
            .await
            .unwrap()
    })
}

#[test]
fn enqueue_does_not_call_the_provider_and_the_queue_is_serial() {
    let (_dir, db, lab, project) = fixture();
    let provider = ScriptedProvider::new(Vec::new());
    let limits = ExecutionLimits::default();
    let clock = TestClock::default();
    let first = requests::enqueue(
        &db,
        &db,
        &project,
        "First request",
        None,
        limits,
        RunId::generate(),
    )
    .unwrap();
    let second = requests::enqueue(
        &db,
        &db,
        &project,
        "Second request",
        None,
        limits,
        RunId::generate(),
    )
    .unwrap();
    assert!(provider.requests().is_empty());
    assert_eq!(status(&db, first.as_str()), "TODO");
    assert_eq!(status(&db, second.as_str()), "TODO");

    // Deterministic ordering even when both inserts share a timestamp.
    db.block_on(async {
        sqlx::query("UPDATE requests SET created_at = ?1 WHERE id = ?2")
            .bind("2026-01-01T00:00:01Z")
            .bind(first.as_str())
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE requests SET created_at = ?1 WHERE id = ?2")
            .bind("2026-01-01T00:00:02Z")
            .bind(second.as_str())
            .execute(db.pool())
            .await
            .unwrap();
    });

    let next = queue::next(&db, &lab).unwrap().unwrap();
    assert_eq!(next.id, first);
    assert_eq!(status(&db, second.as_str()), "TODO");

    provider.push(plan_response());
    let outcome = requests::plan_queued(
        &db,
        &db,
        &next,
        &settings(),
        &provider,
        limits,
        &clock,
        CancelToken::default(),
        RunId::generate(),
    )
    .unwrap();
    assert!(matches!(outcome, PlanningOutcome::Planned { .. }));
    assert_eq!(status(&db, first.as_str()), "PLANNED");
    assert_eq!(status(&db, second.as_str()), "TODO");
    // One planning call plus the best-effort project-memory summary.
    assert_eq!(provider.requests().len(), 2);

    let next = queue::next(&db, &lab).unwrap().unwrap();
    assert_eq!(next.id, second);
    provider.push(plan_response());
    let outcome = requests::plan_queued(
        &db,
        &db,
        &next,
        &settings(),
        &provider,
        limits,
        &clock,
        CancelToken::default(),
        RunId::generate(),
    )
    .unwrap();
    assert!(matches!(outcome, PlanningOutcome::Planned { .. }));
    assert_eq!(status(&db, first.as_str()), "PLANNED");
    assert_eq!(status(&db, second.as_str()), "PLANNED");
    assert_eq!(provider.requests().len(), 4);
    let stories: i64 = db
        .block_on(async {
            sqlx::query_scalar("SELECT count(*) FROM stories")
                .fetch_one(db.pool())
                .await
        })
        .unwrap();
    assert_eq!(stories, 2);
}

#[test]
fn an_enqueued_secret_is_still_rejected_before_persistence() {
    let (_dir, db, _lab, project) = fixture();
    let error = requests::enqueue(
        &db,
        &db,
        &project,
        "sk-vlab-SECRET-MARKER",
        Some("sk-vlab-SECRET-MARKER"),
        ExecutionLimits::default(),
        RunId::generate(),
    )
    .unwrap_err();
    assert!(matches!(error, vlab::app::AppError::Validation { .. }));
    let count: i64 = db
        .block_on(async {
            sqlx::query_scalar("SELECT count(*) FROM requests")
                .fetch_one(db.pool())
                .await
        })
        .unwrap();
    assert_eq!(count, 0);
}
