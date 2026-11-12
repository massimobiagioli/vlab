//! Direct questions and interaction metadata (WP6-AC2/AC10).

mod support;

use vlab::app::ask::{self, AskOutcome};
use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::memories::MemoryStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::budget::{CancelToken, ExecutionLimits};
use vlab::domain::clock::TestClock;
use vlab::domain::ids::{AgentId, LabId, MemoryId};
use vlab::domain::lab::Lab;
use vlab::domain::memory::{Memory, MemoryScope};
use vlab::domain::name::Name;
use vlab::domain::provider::{ProviderError, ProviderResponse};
use vlab::domain::settings::{ModelId, ProviderId, Settings};
use vlab::domain::skills;
use vlab::infra::db::Database;
use vlab::infra::provider::scripted::ScriptedProvider;
use vlab::obs::RunId;

struct Fixture {
    _dir: tempfile::TempDir,
    db: Database,
    lab: LabId,
    lead: AgentId,
}

fn add_agent(db: &Database, lab: &LabId, id: &str, role: Role) -> AgentId {
    let agent = AgentId::parse(id).unwrap();
    db.insert_agent(
        &AgentProfile::new(
            agent.clone(),
            Name::parse("Agent").unwrap(),
            role,
            Description::parse("Does work").unwrap(),
            skills::parse_list("rust").unwrap(),
            ProviderId::DeepSeek,
            ModelId::parse("deepseek-chat").unwrap(),
            Some("CV-MARKER".to_owned()),
        )
        .unwrap(),
        std::slice::from_ref(lab),
    )
    .unwrap();
    agent
}

fn fixture() -> Fixture {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(RunId::generate()).unwrap();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
    let lead = add_agent(&db, &lab, "1c2d3e4f", Role::Lead);
    add_agent(&db, &lab, "2d3e4f50", Role::Dev);
    Fixture {
        _dir: dir,
        db,
        lab,
        lead,
    }
}

fn settings() -> Settings {
    Settings::parse("deepseek", "deepseek-chat").unwrap()
}

fn response(answer: &str) -> ProviderResponse {
    ProviderResponse {
        content: answer.to_owned(),
        model: "deepseek-chat".to_owned(),
        usage: Some(vlab::domain::provider::Usage {
            input_tokens: 3,
            output_tokens: 4,
        }),
        finish_reason: Some("stop".to_owned()),
        request_id: Some("req".to_owned()),
    }
}

fn call(
    fixture: &Fixture,
    provider: &ScriptedProvider,
    to_agent: Option<&str>,
) -> Result<AskOutcome, vlab::app::AppError> {
    ask::ask(
        &fixture.db,
        &fixture.db,
        to_agent,
        "What should we do next?",
        &settings(),
        provider,
        ExecutionLimits::default(),
        CancelToken::default(),
        &TestClock::default(),
        RunId::generate(),
    )
}

#[test]
fn an_agent_target_and_the_default_lead_answer() {
    for target in [Some("2d3e4f50"), None] {
        let fixture = fixture();
        let provider = ScriptedProvider::new(vec![Ok(response("Do the next thing."))]);
        let outcome = call(&fixture, &provider, target).unwrap();
        assert_eq!(outcome.answer, "Do the next thing.");
        let request = &provider.requests()[0];
        assert!(request.user.contains("<question>"));
        assert!(request.user.contains("What should we do next?"));
        assert!(!request.user.contains("CV-MARKER"));
    }
}

#[test]
fn a_non_member_or_missing_lead_fails() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![Ok(response("x"))]);
    let error = call(&fixture, &provider, Some("ffffffff")).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::NotFound { .. }),
        "{error:?}"
    );

    let only_dev = {
        let dir = support::private_tempdir();
        let db = Database::open(dir.path().join("vlab.db")).unwrap();
        db.migrate(RunId::generate()).unwrap();
        let lab = LabId::parse("a3f91c07").unwrap();
        db.insert_lab(&Lab::new(lab.clone(), Name::parse("NoLead").unwrap()))
            .unwrap();
        save_active_lab(&db, &lab).unwrap();
        add_agent(&db, &lab, "2d3e4f50", Role::Dev);
        Fixture {
            _dir: dir,
            db,
            lab,
            lead: AgentId::generate(),
        }
    };
    let error = call(&only_dev, &provider, None).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::NotFound { .. }),
        "{error:?}"
    );
}

#[test]
fn multiple_leads_are_rejected_by_application_validation() {
    let fixture = fixture();
    add_agent(&fixture.db, &fixture.lab, "3e4f5061", Role::Lead);
    let provider = ScriptedProvider::new(vec![Ok(response("x"))]);
    let error = call(&fixture, &provider, None).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::Validation { .. }),
        "{error:?}"
    );
    assert!(provider.requests().is_empty());
}

#[test]
fn a_question_adds_only_interaction_and_event_rows() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![Ok(response("x"))]);
    let before = table_counts(&fixture.db);
    call(&fixture, &provider, None).unwrap();
    let after = table_counts(&fixture.db);
    for table in [
        "labs",
        "agents",
        "lab_memberships",
        "projects",
        "requests",
        "stories",
        "memories",
        "app_state",
    ] {
        assert_eq!(before[index(table)], after[index(table)], "{table}");
    }
    assert_eq!(
        after[index("interactions")],
        before[index("interactions")] + 1
    );
    assert!(after[index("events")] > before[index("events")]);
    let interaction: (String, String, String) = fixture.db.block_on(async {
        sqlx::query_as("SELECT kind, outcome, model FROM interactions LIMIT 1")
            .fetch_one(fixture.db.pool())
            .await
            .unwrap()
    });
    assert_eq!(
        interaction,
        (
            "question".to_owned(),
            "success".to_owned(),
            "deepseek-chat".to_owned()
        )
    );
}

#[test]
fn provider_failure_records_a_failed_interaction() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![Err(ProviderError::Authentication)]);
    let error = call(&fixture, &provider, None).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::Provider(_)),
        "{error:?}"
    );
    let row: (String, String, String) = fixture.db.block_on(async {
        sqlx::query_as("SELECT kind, outcome, error_code FROM interactions LIMIT 1")
            .fetch_one(fixture.db.pool())
            .await
            .unwrap()
    });
    assert_eq!(
        row,
        (
            "question".to_owned(),
            "failure".to_owned(),
            "provider_authentication".to_owned()
        )
    );
}

#[test]
fn question_context_includes_only_lab_and_agent_memories() {
    let fixture = fixture();
    fixture
        .db
        .insert_memory(
            &Memory::new(
                MemoryId::parse("4f506172").unwrap(),
                MemoryScope::Lab(fixture.lab.clone()),
                "lab memory",
            )
            .unwrap(),
        )
        .unwrap();
    fixture
        .db
        .insert_memory(
            &Memory::new(
                MemoryId::parse("5a617283").unwrap(),
                MemoryScope::Agent(fixture.lead.clone()),
                "lead memory",
            )
            .unwrap(),
        )
        .unwrap();
    let provider = ScriptedProvider::new(vec![Ok(response("x"))]);
    call(&fixture, &provider, None).unwrap();
    let user = &provider.requests()[0].user;
    assert!(user.contains("lab memory"));
    assert!(user.contains("lead memory"));
    assert!(!user.contains("unrelated"));
}

fn table_counts(db: &Database) -> Vec<i64> {
    [
        "labs",
        "agents",
        "lab_memberships",
        "projects",
        "requests",
        "stories",
        "memories",
        "app_state",
        "interactions",
        "events",
    ]
    .into_iter()
    .map(|table| {
        let sql = format!("SELECT count(*) FROM {table}");
        db.block_on(async {
            sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_one(db.pool())
                .await
                .unwrap()
        })
    })
    .collect()
}

fn index(table: &str) -> usize {
    [
        "labs",
        "agents",
        "lab_memberships",
        "projects",
        "requests",
        "stories",
        "memories",
        "app_state",
        "interactions",
        "events",
    ]
    .iter()
    .position(|candidate| *candidate == table)
    .unwrap()
}
