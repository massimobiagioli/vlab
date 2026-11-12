//! Cancellation, timeouts, and budget exhaustion (WP5-AC10).

mod support;

use std::cell::Cell;
use std::time::Duration;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::app::requests::{self, PlanningOutcome};
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::budget::{CancelToken, ExecutionLimits};
use vlab::domain::clock::TestClock;
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::provider::{ModelProvider, ProviderError, ProviderRequest, ProviderResponse};
use vlab::domain::settings::{ModelId, ProviderId, Settings};
use vlab::domain::skills;
use vlab::infra::db::Database;
use vlab::infra::provider::scripted::ScriptedProvider;
use vlab::obs::RunId;

struct Fixture {
    _dir: tempfile::TempDir,
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
        _dir: dir,
        db,
        project,
    }
}

fn plan(
    fixture: &Fixture,
    provider: &impl ModelProvider,
    limits: ExecutionLimits,
    cancel: CancelToken,
) -> PlanningOutcome {
    requests::plan(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        "Do work",
        &Settings::parse("deepseek", "deepseek-chat").unwrap(),
        provider,
        limits,
        &TestClock::default(),
        cancel,
        RunId::generate(),
    )
    .unwrap()
}

struct CancellingProvider {
    cancel: CancelToken,
    calls: Cell<u32>,
}

struct CancellingSuccessProvider {
    cancel: CancelToken,
}

impl ModelProvider for CancellingSuccessProvider {
    fn complete(&self, _: &ProviderRequest) -> Result<ProviderResponse, ProviderError> {
        self.cancel.cancel();
        Ok(ProviderResponse {
            content: r#"{"schema_version":1,"clarification_questions":["Why?"],"stories":[]}"#
                .to_owned(),
            model: "m".to_owned(),
            usage: None,
            finish_reason: None,
            request_id: None,
        })
    }
}

impl ModelProvider for CancellingProvider {
    fn complete(&self, _: &ProviderRequest) -> Result<ProviderResponse, ProviderError> {
        self.calls.set(self.calls.get() + 1);
        self.cancel.cancel();
        Err(ProviderError::RateLimited { retry_after: None })
    }
}

#[test]
fn cancellation_before_dispatch_performs_no_call() {
    let fixture = fixture();
    let cancel = CancelToken::default();
    cancel.cancel();
    let provider = ScriptedProvider::with_cancel(cancel.clone());
    let outcome = plan(&fixture, &provider, ExecutionLimits::default(), cancel);
    assert!(matches!(outcome, PlanningOutcome::Cancelled { .. }));
    assert!(provider.requests().is_empty());
}

#[test]
fn cancellation_during_dispatch_yields_one_attempt_without_retry() {
    let fixture = fixture();
    let cancel = CancelToken::default();
    let provider = CancellingProvider {
        cancel: cancel.clone(),
        calls: Cell::new(0),
    };
    let outcome = plan(&fixture, &provider, ExecutionLimits::default(), cancel);
    assert!(matches!(outcome, PlanningOutcome::Cancelled { .. }));
    assert_eq!(provider.calls.get(), 1);
}

#[test]
fn a_response_arriving_after_cancellation_is_not_committed() {
    let fixture = fixture();
    let cancel = CancelToken::default();
    let provider = CancellingSuccessProvider {
        cancel: cancel.clone(),
    };
    let outcome = plan(&fixture, &provider, ExecutionLimits::default(), cancel);
    assert!(matches!(outcome, PlanningOutcome::Cancelled { .. }));
    let count: i64 = fixture.db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM stories")
            .fetch_one(fixture.db.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 0);
}

#[test]
fn timeouts_are_classified_and_bounded() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![
        Err(ProviderError::Timeout),
        Err(ProviderError::Timeout),
        Err(ProviderError::Timeout),
        Err(ProviderError::Timeout),
    ]);
    let outcome = plan(
        &fixture,
        &provider,
        ExecutionLimits::default(),
        CancelToken::default(),
    );
    let PlanningOutcome::Failed {
        code, validation, ..
    } = outcome
    else {
        panic!("unexpected outcome: {outcome:?}");
    };
    assert_eq!(code, "provider_timeout");
    assert!(!validation);
    assert_eq!(provider.requests().len(), 3);
    let attempts: u32 = fixture.db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM events WHERE kind = 'provider_attempt'")
            .fetch_one(fixture.db.pool())
            .await
            .unwrap()
    });
    assert_eq!(attempts, 3);
}

#[test]
fn an_expired_total_deadline_reports_budget_exceeded() {
    let fixture = fixture();
    let provider = ScriptedProvider::new(vec![Err(ProviderError::RateLimited {
        retry_after: Some(Duration::from_secs(10)),
    })]);
    let outcome = plan(
        &fixture,
        &provider,
        ExecutionLimits {
            total_deadline: Duration::from_secs(5),
            ..ExecutionLimits::default()
        },
        CancelToken::default(),
    );
    let PlanningOutcome::Failed { code, .. } = outcome else {
        panic!("unexpected outcome: {outcome:?}");
    };
    assert_eq!(code, "budget_exceeded");
    assert_eq!(provider.requests().len(), 1);
}
